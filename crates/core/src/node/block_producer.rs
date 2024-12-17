use crate::node::pool::{TxBatch, TxPool};
use crate::node::sealer::BlockSealer;
use crate::node::InMemoryNode;
use crate::system_contracts::SystemContracts;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use zksync_multivm::interface::TxExecutionMode;

pub struct BlockProducer {
    node: InMemoryNode,
    pool: TxPool,
    block_sealer: BlockSealer,
    system_contracts: SystemContracts,
}

impl BlockProducer {
    pub fn new(
        node: InMemoryNode,
        pool: TxPool,
        block_sealer: BlockSealer,
        system_contracts: SystemContracts,
    ) -> Self {
        Self {
            node,
            pool,
            block_sealer,
            system_contracts,
        }
    }
}

impl Future for BlockProducer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pin = self.get_mut();

        while let Poll::Ready(tx_batch) = pin.block_sealer.poll(&pin.pool, cx) {
            let TxBatch { impersonating, txs } = tx_batch;

            let base_system_contracts = pin
                .system_contracts
                .contracts(TxExecutionMode::VerifyExecute, impersonating)
                .clone();
            pin.node
                .seal_block(&mut pin.node.time.lock(), txs, base_system_contracts)
                .expect("block sealing failed");
        }

        Poll::Pending
    }
}
