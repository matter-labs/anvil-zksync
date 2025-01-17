use std::collections::HashMap;

use bigdecimal::BigDecimal;
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;
use zksync_types::{
    api::{
        BlockDetails, BridgeAddresses, L1BatchDetails, L2ToL1LogProof, Proof, ProtocolVersion,
        TransactionDetails,
    },
    fee::Fee,
    transaction_request::CallRequest,
    Address, L1BatchNumber, L2BlockNumber, H256, U256, U64,
};
use zksync_web3_decl::types::Token;

#[rpc]
pub trait ZksNamespaceT {
    #[rpc(name = "zks_estimateFee")]
    fn estimate_fee(&self, req: CallRequest) -> BoxFuture<Result<Fee>>;

    #[rpc(name = "zks_estimateGasL1ToL2")]
    fn estimate_gas_l1_to_l2(&self, req: CallRequest) -> BoxFuture<Result<U256>>;

    #[rpc(name = "zks_getMainContract")]
    fn get_main_contract(&self) -> BoxFuture<Result<Address>>;

    #[rpc(name = "zks_getTestnetPaymaster")]
    fn get_testnet_paymaster(&self) -> BoxFuture<Result<Option<Address>>>;

    #[rpc(name = "zks_getBridgeContracts")]
    fn get_bridge_contracts(&self) -> BoxFuture<Result<BridgeAddresses>>;

    #[rpc(name = "zks_L1ChainId")]
    fn l1_chain_id(&self) -> BoxFuture<Result<U64>>;

    #[rpc(name = "zks_getConfirmedTokens")]
    fn get_confirmed_tokens(&self, from: u32, limit: u8) -> BoxFuture<Result<Vec<Token>>>;

    #[rpc(name = "zks_getTokenPrice")]
    fn get_token_price(&self, token_address: Address) -> BoxFuture<Result<BigDecimal>>;

    #[rpc(name = "zks_getAllAccountBalances")]
    fn get_all_account_balances(
        &self,
        address: Address,
    ) -> BoxFuture<Result<HashMap<Address, U256>>>;

    #[rpc(name = "zks_getL2ToL1MsgProof")]
    fn get_l2_to_l1_msg_proof(
        &self,
        block: L2BlockNumber,
        sender: Address,
        msg: H256,
        l2_log_position: Option<usize>,
    ) -> BoxFuture<Result<Option<L2ToL1LogProof>>>;

    #[rpc(name = "zks_getL2ToL1LogProof")]
    fn get_l2_to_l1_log_proof(
        &self,
        tx_hash: H256,
        index: Option<usize>,
    ) -> BoxFuture<Result<Option<L2ToL1LogProof>>>;

    #[rpc(name = "zks_L1BatchNumber")]
    fn get_l1_batch_number(&self) -> BoxFuture<Result<U64>>;

    #[rpc(name = "zks_getBlockDetails")]
    fn get_block_details(
        &self,
        block_number: L2BlockNumber,
    ) -> BoxFuture<Result<Option<BlockDetails>>>;

    #[rpc(name = "zks_getL1BatchBlockRange")]
    fn get_miniblock_range(&self, batch: L1BatchNumber) -> BoxFuture<Result<Option<(U64, U64)>>>;

    #[rpc(name = "zks_getTransactionDetails")]
    fn get_transaction_details(&self, hash: H256) -> BoxFuture<Result<Option<TransactionDetails>>>;

    #[rpc(name = "zks_getRawBlockTransactions")]
    fn get_raw_block_transactions(
        &self,
        block_number: L2BlockNumber,
    ) -> BoxFuture<Result<Vec<zksync_types::Transaction>>>;

    #[rpc(name = "zks_getL1BatchDetails")]
    fn get_l1_batch_details(
        &self,
        batch: L1BatchNumber,
    ) -> BoxFuture<Result<Option<L1BatchDetails>>>;

    #[rpc(name = "zks_getBytecodeByHash")]
    fn get_bytecode_by_hash(&self, hash: H256) -> BoxFuture<Result<Option<Vec<u8>>>>;

    #[rpc(name = "zks_getL1GasPrice")]
    fn get_l1_gas_price(&self) -> BoxFuture<Result<U64>>;

    #[rpc(name = "zks_getProtocolVersion")]
    fn get_protocol_version(
        &self,
        version_id: Option<u16>,
    ) -> BoxFuture<Result<Option<ProtocolVersion>>>;

    #[rpc(name = "zks_getProof")]
    fn get_proof(
        &self,
        address: Address,
        keys: Vec<H256>,
        l1_batch_number: L1BatchNumber,
    ) -> BoxFuture<Result<Proof>>;

    #[rpc(name = "zks_getBaseTokenL1Address")]
    fn get_base_token_l1_address(&self) -> BoxFuture<Result<Address>>;
}
