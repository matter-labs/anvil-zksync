#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(allocator_api)]
#![feature(array_chunks)]
#![feature(get_mut_unchecked)]
#![feature(const_type_id)]
#![feature(vec_push_within_capacity)]
#![feature(ptr_alignment_type)]
#![feature(btreemap_alloc)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(ptr_metadata)]
#![feature(alloc_layout_extra)]
#![feature(array_windows)]
#![feature(btree_cursors)]
#![feature(slice_from_ptr_range)]
#![feature(const_trait_impl)]
#![allow(clippy::new_without_default)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::double_must_use)]
#![allow(clippy::explicit_auto_deref)]

use crate::anvil::AnvilHandle;
use crate::commitment_generator::CommitmentGenerator;
use crate::l1_executor::L1Executor;
use crate::l1_sender::{L1Sender, L1SenderHandle};
use crate::l1_watcher::L1Watcher;
use crate::upgrade_tx::UpgradeTx;
use crate::zkstack_config::contracts::ContractsConfig;
use crate::zkstack_config::genesis::GenesisConfig;
use crate::zkstack_config::ZkstackConfig;
use alloy::providers::Provider;
use anvil_zksync_core::node::blockchain::ReadBlockchain;
use anvil_zksync_core::node::node_executor::NodeExecutorHandle;
use anvil_zksync_core::node::{TxBatch, TxPool};
use std::sync::Arc;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use zksync_types::protocol_upgrade::ProtocolUpgradeTxCommonData;
use zksync_types::{
    ExecuteTransactionCommon, L1BatchNumber, ProtocolVersionId, Transaction, H256, U256,
};

mod anvil;
mod commitment_generator;
mod contracts;
mod l1_executor;
mod l1_sender;
mod l1_watcher;
mod upgrade_tx;
mod zkstack_config;

#[derive(Debug, Clone)]
pub struct L1Sidecar {
    inner: Option<L1SidecarInner>,
}

#[derive(Debug, Clone)]
struct L1SidecarInner {
    commitment_generator: CommitmentGenerator,
    l1_sender_handle: L1SenderHandle,
    zkstack_config: ZkstackConfig,
}

impl L1Sidecar {
    pub fn none() -> Self {
        Self { inner: None }
    }

    async fn new(
        blockchain: Box<dyn ReadBlockchain>,
        node_handle: NodeExecutorHandle,
        pool: TxPool,
        zkstack_config: ZkstackConfig,
        anvil_handle: AnvilHandle,
        anvil_provider: Arc<dyn Provider + 'static>,
        auto_execute_l1: bool,
    ) -> anyhow::Result<(Self, L1SidecarRunner)> {
        let commitment_generator = CommitmentGenerator::new(&zkstack_config, blockchain);
        let genesis_with_metadata = commitment_generator
            .get_or_generate_metadata(L1BatchNumber(0))
            .await
            .ok_or(anyhow::anyhow!(
                "genesis is missing from local storage, can't start L1 sidecar"
            ))?;
        let (l1_sender, l1_sender_handle) = L1Sender::new(
            &zkstack_config,
            genesis_with_metadata,
            anvil_provider.clone(),
        );
        let l1_watcher = L1Watcher::new(&zkstack_config, anvil_provider, pool);
        let protocol_version = zkstack_config.genesis.genesis_protocol_version;
        let l1_executor = if auto_execute_l1 {
            L1Executor::auto(commitment_generator.clone(), l1_sender_handle.clone())
        } else {
            L1Executor::manual()
        };
        let this = Self {
            inner: Some(L1SidecarInner {
                commitment_generator,
                l1_sender_handle,
                zkstack_config,
            }),
        };
        let upgrade_handle = tokio::spawn(Self::upgrade(protocol_version, node_handle));
        let runner = L1SidecarRunner {
            anvil_handle,
            l1_sender,
            l1_watcher,
            l1_executor,
            upgrade_handle,
        };
        Ok((this, runner))
    }

    pub async fn process(
        protocol_version: ProtocolVersionId,
        port: u16,
        blockchain: Box<dyn ReadBlockchain>,
        node_handle: NodeExecutorHandle,
        pool: TxPool,
        auto_execute_l1: bool,
    ) -> anyhow::Result<(Self, L1SidecarRunner)> {
        let zkstack_config = ZkstackConfig::builtin(protocol_version);
        let (anvil_handle, anvil_provider) = anvil::spawn_process(port, &zkstack_config).await?;
        Self::new(
            blockchain,
            node_handle,
            pool,
            zkstack_config,
            anvil_handle,
            anvil_provider,
            auto_execute_l1,
        )
        .await
    }

    pub async fn external(
        protocol_version: ProtocolVersionId,
        address: &str,
        blockchain: Box<dyn ReadBlockchain>,
        node_handle: NodeExecutorHandle,
        pool: TxPool,
        auto_execute_l1: bool,
    ) -> anyhow::Result<(Self, L1SidecarRunner)> {
        let zkstack_config = ZkstackConfig::builtin(protocol_version);
        let (anvil_handle, anvil_provider) = anvil::external(address, &zkstack_config).await?;
        Self::new(
            blockchain,
            node_handle,
            pool,
            zkstack_config,
            anvil_handle,
            anvil_provider,
            auto_execute_l1,
        )
        .await
    }

    /// Clean L1 always expects the very first transaction to upgrade system contracts. Thus, L1
    /// sidecar has to be initialized before any other component that can submit transactions.
    async fn upgrade(
        protocol_version: ProtocolVersionId,
        node_handle: NodeExecutorHandle,
    ) -> anyhow::Result<()> {
        let upgrade_tx = UpgradeTx::builtin(protocol_version);
        tracing::info!(
            tx_hash = ?upgrade_tx.hash,
            initiator_address = ?upgrade_tx.initiator_address,
            contract_address = ?upgrade_tx.data.contract_address,
            "executing upgrade transaction"
        );
        let upgrade_tx = Transaction {
            common_data: ExecuteTransactionCommon::ProtocolUpgrade(ProtocolUpgradeTxCommonData {
                sender: upgrade_tx.initiator_address,
                upgrade_id: protocol_version,
                max_fee_per_gas: U256::from(upgrade_tx.max_fee_per_gas),
                gas_limit: U256::from(upgrade_tx.gas_limit),
                gas_per_pubdata_limit: U256::from(upgrade_tx.gas_per_pubdata_limit),
                eth_block: upgrade_tx.l1_block_number,
                canonical_tx_hash: upgrade_tx.hash,
                to_mint: U256::from(upgrade_tx.l1_tx_mint),
                refund_recipient: upgrade_tx.l1_tx_refund_recipient,
            }),
            execute: upgrade_tx.data,
            received_timestamp_ms: 0,
            raw_bytes: None,
        };
        let upgrade_block = node_handle
            .seal_block_sync(TxBatch {
                impersonating: false,
                txs: vec![upgrade_tx],
            })
            .await?;
        tracing::info!(%upgrade_block, "upgrade finished successfully");
        Ok(())
    }
}

pub struct L1SidecarRunner {
    anvil_handle: AnvilHandle,
    l1_sender: L1Sender,
    l1_watcher: L1Watcher,
    l1_executor: L1Executor,
    upgrade_handle: JoinHandle<anyhow::Result<()>>,
}

impl L1SidecarRunner {
    pub async fn run(self) -> anyhow::Result<()> {
        // We ensure L2 upgrade finishes before the rest of L1 logic can be run.
        self.upgrade_handle.await??;
        let (_stop_sender, mut stop_receiver) = watch::channel(false);
        tokio::select! {
            result = self.l1_sender.run() => {
                tracing::trace!("L1 sender was stopped");
                result
            },
            result = self.l1_watcher.run() => {
                tracing::trace!("L1 watcher was stopped");
                result
            },
            result = self.anvil_handle.run() => {
                tracing::trace!("L1 anvil exited unexpectedly");
                result
            },
            result = self.l1_executor.run(&mut stop_receiver) => result,
        }
    }
}

impl L1Sidecar {
    pub async fn commit_batch(&self, batch_number: L1BatchNumber) -> anyhow::Result<H256> {
        let Some(inner) = self.inner.as_ref() else {
            return Err(anyhow::anyhow!(
                "cannot commit a batch as there is no L1 configured"
            ));
        };
        let batch_with_metadata = inner
            .commitment_generator
            .get_or_generate_metadata(batch_number)
            .await
            .ok_or_else(|| anyhow::anyhow!("batch #{batch_number} does not exist"))?;
        inner
            .l1_sender_handle
            .commit_sync(batch_with_metadata)
            .await
    }

    pub async fn prove_batch(&self, batch_number: L1BatchNumber) -> anyhow::Result<H256> {
        let Some(inner) = self.inner.as_ref() else {
            return Err(anyhow::anyhow!(
                "cannot prove a batch as there is no L1 configured"
            ));
        };
        let batch_with_metadata = inner
            .commitment_generator
            .get_or_generate_metadata(batch_number)
            .await
            .ok_or_else(|| anyhow::anyhow!("batch #{batch_number} does not exist"))?;
        inner.l1_sender_handle.prove_sync(batch_with_metadata).await
    }

    pub async fn execute_batch(&self, batch_number: L1BatchNumber) -> anyhow::Result<H256> {
        let Some(inner) = self.inner.as_ref() else {
            return Err(anyhow::anyhow!(
                "cannot execute a batch as there is no L1 configured"
            ));
        };
        let batch_with_metadata = inner
            .commitment_generator
            .get_or_generate_metadata(batch_number)
            .await
            .ok_or_else(|| anyhow::anyhow!("batch #{batch_number} does not exist"))?;
        inner
            .l1_sender_handle
            .execute_sync(batch_with_metadata)
            .await
    }

    pub fn contracts_config(&self) -> anyhow::Result<&ContractsConfig> {
        let Some(inner) = self.inner.as_ref() else {
            return Err(anyhow::anyhow!(
                "cannot get contracts config as there is no L1 configured"
            ));
        };
        Ok(&inner.zkstack_config.contracts)
    }

    pub fn genesis_config(&self) -> anyhow::Result<&GenesisConfig> {
        let Some(inner) = self.inner.as_ref() else {
            return Err(anyhow::anyhow!(
                "cannot get genesis config as there is no L1 configured"
            ));
        };
        Ok(&inner.zkstack_config.genesis)
    }
}
