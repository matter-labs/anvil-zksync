use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use zksync_types::web3::Bytes;
use zksync_types::{L1BatchNumber, H256};

/// Custom namespace that contains anvil-zksync specific methods.
#[rpc(server, namespace = "anvil_zks")]
pub trait AnvilZksNamespace {
    /// Commit batch's metadata to L1 (if there is one).
    ///
    /// Assumes L1 contracts have no system log verification and no DA input verification.
    ///
    /// # Arguments
    ///
    /// * `batch_number` - Number of the batch to be committed
    ///
    /// # Returns
    /// Finalized L1 transaction's hash that successfully commited the batch.
    #[method(name = "commitBatch")]
    async fn commit_batch(&self, batch_number: L1BatchNumber) -> RpcResult<H256>;

    /// Send batch's proof to L1 (if there is one). Batch needs to be committed first.
    ///
    /// Assumes L1 contracts allow empty proofs (see `TestnetVerifier.sol`).
    ///
    /// # Arguments
    ///
    /// * `batch_number` - Number of the batch to be proved
    ///
    /// # Returns
    /// Finalized L1 transaction's hash that successfully proved the batch.
    #[method(name = "proveBatch")]
    async fn prove_batch(&self, batch_number: L1BatchNumber) -> RpcResult<H256>;

    /// Execute batch on L1 (if there is one). Batch needs to be committed and proved first.
    ///
    /// # Arguments
    ///
    /// * `batch_number` - Number of the batch to be executed
    ///
    /// # Returns
    /// Finalized L1 transaction's hash that successfully executed the batch.
    #[method(name = "executeBatch")]
    async fn execute_batch(&self, batch_number: L1BatchNumber) -> RpcResult<H256>;

    /// Returns the witness for a given batch.
    ///
    /// # Arguments
    ///
    /// * `batch_number` - Number of the batch to return witness for
    ///
    /// # Returns
    /// Bytes with the witness that can be passed to proving system.
    #[method(name = "getZKsyncOSWitness")]
    async fn get_zksync_os_witness(&self, batch_number: L1BatchNumber) -> RpcResult<Bytes>;
}
