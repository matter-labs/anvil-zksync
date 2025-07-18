use crate::contracts::NewPriorityRequest;
use crate::zkstack_config::ZkstackConfig;
use alloy::eips::BlockId;
use alloy::providers::Provider;
use alloy::rpc::types::Filter;
use alloy::sol_types::SolEvent;
use anvil_zksync_core::node::TxPool;
use anyhow::Context;
use std::sync::Arc;
use std::time::Duration;
use zksync_types::l1::L1Tx;
use zksync_types::{L2_MESSAGE_ROOT_ADDRESS, PriorityOpId};

/// Node component responsible for saving new priority L1 transactions to transaction pool.
pub struct L1Watcher {
    provider: Arc<dyn Provider + 'static>,
    pool: TxPool,
    addresses: Vec<alloy::primitives::Address>,

    next_expected_priority_id: PriorityOpId,
    from_block: u64,
}

impl L1Watcher {
    pub fn new(
        zkstack_config: &ZkstackConfig,
        provider: Arc<dyn Provider + 'static>,
        pool: TxPool,
    ) -> Self {
        let addresses = vec![
            alloy::primitives::Address::from(zkstack_config.contracts.l1.diamond_proxy_addr.0),
            alloy::primitives::Address::from(zkstack_config.contracts.l1.governance_addr.0),
            alloy::primitives::Address::from(
                zkstack_config
                    .contracts
                    .ecosystem_contracts
                    .state_transition_proxy_addr
                    .0,
            ),
            alloy::primitives::Address::from(zkstack_config.contracts.l1.chain_admin_addr.0),
            alloy::primitives::Address::from(L2_MESSAGE_ROOT_ADDRESS.0),
        ];
        Self {
            provider,
            pool,
            addresses,
            next_expected_priority_id: PriorityOpId(0),
            from_block: 0,
        }
    }

    /// Runs L1 watcher indefinitely thus saving all incoming L1 transaction to the pool.
    pub async fn run(mut self) -> anyhow::Result<()> {
        let mut timer = tokio::time::interval(Duration::from_millis(100));
        loop {
            timer.tick().await;
            self.poll().await?;
        }
    }
}

impl L1Watcher {
    async fn poll(&mut self) -> anyhow::Result<()> {
        let latest_block = self
            .provider
            .get_block(BlockId::latest())
            .await?
            .context("L1 does not have any block")?;
        let to_block = latest_block.header.number;
        if self.from_block > to_block {
            return Ok(());
        }
        let filter = Filter::new()
            .from_block(self.from_block)
            .to_block(to_block)
            .event_signature(NewPriorityRequest::SIGNATURE_HASH)
            .address(self.addresses.clone());
        let events = self.provider.get_logs(&filter).await?;
        let mut priority_txs = Vec::new();
        for event in events {
            let zksync_log: zksync_types::web3::Log =
                serde_json::from_value(serde_json::to_value(event)?)?;
            let tx = L1Tx::try_from(zksync_log)?;
            priority_txs.push(tx);
        }

        if priority_txs.is_empty() {
            return Ok(());
        }
        // unwraps are safe because the vec is not empty
        let first = priority_txs.first().unwrap();
        let last = priority_txs.last().unwrap();
        tracing::info!(
            first_serial_id = %first.serial_id(),
            last_serial_id = %last.serial_id(),
            first_block = %first.eth_block(),
            last_block = %last.eth_block(),
            "received priority requests",
        );
        anyhow::ensure!(
            last.serial_id().0 - first.serial_id().0 + 1 == priority_txs.len() as u64,
            "there is a gap in priority transactions received"
        );
        let new_txs: Vec<_> = priority_txs
            .into_iter()
            .skip_while(|tx| tx.serial_id() < self.next_expected_priority_id)
            .collect();

        if new_txs.is_empty() {
            return Ok(());
        }
        let first = new_txs.first().unwrap();
        let last = new_txs.last().unwrap();
        anyhow::ensure!(
            first.serial_id() == self.next_expected_priority_id,
            "priority transaction serial id mismatch"
        );

        let next_expected_priority_id = last.serial_id().next();
        for tx in new_txs {
            tracing::debug!(
                hash = ?tx.hash(),
                "adding new priority transaction to mempool",
            );
            self.pool.add_tx(tx.into());
        }
        self.next_expected_priority_id = next_expected_priority_id;
        self.from_block = to_block + 1;

        Ok(())
    }
}
