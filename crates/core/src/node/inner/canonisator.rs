use tokio::sync::mpsc;
use zk_os_forward_system::run::BatchOutput;
use zksync_error::anvil_zksync;
use zksync_error::anvil_zksync::node::{AnvilNodeError, AnvilNodeResult};
use zksync_os_sequencer::model::ReplayRecord;
use zksync_os_sequencer::storage::block_replay_storage::BlockReplayStorage;
use zksync_os_sequencer::storage::StateHandle;

pub struct Canonisator {
    block_replay_storage: BlockReplayStorage,
    state_handle: StateHandle,
    command_receiver: mpsc::Receiver<Command>,
}

impl Canonisator {
    pub fn new(
        state_handle: StateHandle,
        block_replay_storage: BlockReplayStorage,
    ) -> (Self, CanonisatorHandle) {
        let (command_sender, command_receiver) = mpsc::channel(128);
        let this = Self {
            state_handle,
            block_replay_storage,
            command_receiver,
        };
        let handle = CanonisatorHandle { command_sender };
        (this, handle)
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        while let Some(command) = self.command_receiver.recv().await {
            tracing::info!("canonise loop start");
            match command {
                Command::Canonise(batch_output, replay) => {
                    let bn = batch_output.header.number;
                    tracing::info!(block = bn, "▶ append_replay");
                    self.block_replay_storage
                        .append_replay(batch_output.clone(), replay)
                        .await;

                    tracing::info!(block = bn, "▶ advance_canonized_block");
                    self.state_handle.advance_canonized_block(bn);
                    tracing::info!(block = bn, "✔ done");
                }
            }
            tracing::info!("canonise loop end");
        }

        tracing::trace!("channel has been closed; stopping canonisator");
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CanonisatorHandle {
    command_sender: mpsc::Sender<Command>,
}

impl CanonisatorHandle {
    pub async fn canonise(
        &self,
        batch_output: BatchOutput,
        replay: ReplayRecord,
    ) -> AnvilNodeResult<()> {
        execute_without_response(
            &self.command_sender,
            Command::Canonise(batch_output, replay),
        )
        .await
    }
}

async fn execute_without_response(
    command_sender: &mpsc::Sender<Command>,
    command: Command,
) -> AnvilNodeResult<()> {
    let action_name = command.readable_action_name();
    command_sender
        .send(command)
        .await
        .map_err(|_| canonisator_dropped_error("request to", &action_name))
}

fn canonisator_dropped_error(request_or_receive: &str, action_name: &str) -> AnvilNodeError {
    anvil_zksync::node::generic_error!(
        r"Failed to {request_or_receive} {action_name} because canonisator is dropped. \
         Another error was likely propagated from the main execution loop. \
         If this is not the case, please, report this as a bug."
    )
}

#[derive(Debug)]
enum Command {
    Canonise(BatchOutput, ReplayRecord),
}

impl Command {
    /// Human-readable command description used for diagnostics.
    fn readable_action_name(&self) -> String {
        match self {
            Command::Canonise(batch_output, replay) => {
                format!(
                    "canonise block #{} with {} transactions",
                    batch_output.header.number,
                    replay.transactions.len()
                )
            }
        }
    }
}
