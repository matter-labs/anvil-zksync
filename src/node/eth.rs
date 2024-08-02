use std::collections::HashSet;

use colored::Colorize;
use futures::FutureExt;
use itertools::Itertools;
use multivm::interface::{ExecutionResult, TxExecutionMode};
use multivm::vm_latest::constants::ETH_CALL_GAS_LIMIT;
use zksync_basic_types::{
    web3::{self, Bytes},
    AccountTreeId, Address, H160, H256, U256, U64,
};
use zksync_types::{
    api::{Block, BlockIdVariant, BlockNumber, TransactionVariant},
    fee::Fee,
    get_code_key, get_nonce_key,
    l2::L2Tx,
    transaction_request::TransactionRequest,
    utils::storage_key_for_standard_token_balance,
    PackedEthSignature, StorageKey, L2_BASE_TOKEN_ADDRESS, MAX_L1_TRANSACTION_GAS_LIMIT,
};
use zksync_utils::{h256_to_u256, u256_to_h256};
use zksync_web3_decl::{
    error::Web3Error,
    types::{FeeHistory, Filter, FilterChanges, SyncState},
};

use crate::{
    filters::{FilterType, LogFilter},
    fork::ForkSource,
    namespaces::{EthNamespaceT, EthTestNodeNamespaceT, RpcResult},
    node::{InMemoryNode, TransactionResult, MAX_TX_SIZE, PROTOCOL_VERSION},
    utils::{
        self, h256_to_u64, into_jsrpc_error, into_jsrpc_error_message, not_implemented,
        report_into_jsrpc_error, IntoBoxedFuture,
    },
};

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> EthNamespaceT
    for InMemoryNode<S>
{
    /// Returns the chain ID of the node.
    fn chain_id(&self) -> RpcResult<zksync_basic_types::U64> {
        match self.get_inner().read() {
            Ok(inner) => Ok(U64::from(inner.fork_storage.chain_id.as_u64())).into_boxed_future(),
            Err(_) => Err(into_jsrpc_error(Web3Error::InternalError(
                anyhow::Error::msg("Failed to acquire read lock for chain ID retrieval"),
            )))
            .into_boxed_future(),
        }
    }

    /// Calls the specified function on the L2 contract with the given arguments.
    ///
    /// # Arguments
    ///
    /// * `req` - The call request containing the function name and arguments.
    /// * `_block` - The block ID variant (unused).
    ///
    /// # Returns
    ///
    /// A boxed future containing the result of the function call.
    fn call(
        &self,
        req: zksync_types::transaction_request::CallRequest,
        _block: Option<BlockIdVariant>,
    ) -> RpcResult<Bytes> {
        match L2Tx::from_request(req.into(), MAX_TX_SIZE) {
            Ok(mut tx) => {
                tx.common_data.fee.gas_limit = ETH_CALL_GAS_LIMIT.into();
                let result = self.run_l2_call(tx);

                match result {
                    Ok(execution_result) => match execution_result {
                        ExecutionResult::Success { output } => {
                            Ok(output.into()).into_boxed_future()
                        }
                        ExecutionResult::Revert { output } => {
                            let message = output.to_user_friendly_string();
                            let pretty_message = format!(
                                "execution reverted{}{}",
                                if message.is_empty() { "" } else { ": " },
                                message
                            );

                            tracing::info!("{}", pretty_message.on_red());
                            Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                                pretty_message,
                                output.encoded_data(),
                            )))
                            .into_boxed_future()
                        }
                        ExecutionResult::Halt { reason } => {
                            let message = reason.to_string();
                            let pretty_message = format!(
                                "execution halted {}{}",
                                if message.is_empty() { "" } else { ": " },
                                message
                            );

                            tracing::info!("{}", pretty_message.on_red());
                            Err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                                pretty_message,
                                vec![],
                            )))
                            .into_boxed_future()
                        }
                    },
                    Err(e) => {
                        let error_message = format!("Invalid data due to invalid name: {}", e);
                        let error = Web3Error::InternalError(anyhow::Error::msg(error_message));
                        Err(into_jsrpc_error(error)).into_boxed_future()
                    }
                }
            }
            Err(e) => {
                let error = Web3Error::SerializationError(e);
                Err(into_jsrpc_error(error)).into_boxed_future()
            }
        }
    }

    /// Returns the balance of the specified address.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to get the balance of.
    /// * `_block` - The block ID variant (optional).
    ///
    /// # Returns
    ///
    /// A `BoxFuture` that resolves to a `Result` containing the balance of the specified address as a `U256` or a `jsonrpc_core::Error` if an error occurred.
    fn get_balance(&self, address: Address, _block: Option<BlockIdVariant>) -> RpcResult<U256> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let balance_key = storage_key_for_standard_token_balance(
                AccountTreeId::new(L2_BASE_TOKEN_ADDRESS),
                &address,
            );

            match inner.write() {
                Ok(inner_guard) => {
                    match inner_guard.fork_storage.read_value_internal(&balance_key) {
                        Ok(balance) => Ok(h256_to_u256(balance)),
                        Err(error) => Err(report_into_jsrpc_error(error)),
                    }
                }
                Err(_) => {
                    let error_message = "Error acquiring write lock for balance retrieval";
                    let web3_error = Web3Error::InternalError(anyhow::Error::msg(error_message));
                    Err(into_jsrpc_error(web3_error))
                }
            }
        })
    }

    /// Returns a block by its number.
    ///
    /// # Arguments
    ///
    /// * `block_number` - A `BlockNumber` enum variant representing the block number to retrieve.
    /// * `full_transactions` - A boolean value indicating whether to retrieve full transactions or not.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to an `Option` of `Block<TransactionVariant>`.
    fn get_block_by_number(
        &self,
        block_number: BlockNumber,
        full_transactions: bool,
    ) -> RpcResult<Option<Block<TransactionVariant>>> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let maybe_block = {
                let reader = match inner.read() {
                    Ok(r) => r,
                    Err(_) => {
                        return Err(into_jsrpc_error(Web3Error::InternalError(
                            anyhow::Error::msg("Failed to acquire read lock for block retrieval"),
                        )))
                    }
                };
                let number =
                    utils::to_real_block_number(block_number, U64::from(reader.current_miniblock))
                        .as_u64();

                reader
                    .block_hashes
                    .get(&number)
                    .and_then(|hash| reader.blocks.get(hash))
                    .cloned()
                    .or_else(|| {
                        reader
                            .fork_storage
                            .inner
                            .read()
                            .expect("failed reading fork storage")
                            .fork
                            .as_ref()
                            .and_then(|fork| {
                                fork.fork_source
                                    .get_block_by_number(block_number, true)
                                    .ok()
                                    .flatten()
                            })
                    })
            };

            match maybe_block {
                Some(mut block) => {
                    let block_hash = block.hash;
                    block.transactions = block
                        .transactions
                        .into_iter()
                        .map(|transaction| match &transaction {
                            TransactionVariant::Full(inner) => {
                                if full_transactions {
                                    transaction
                                } else {
                                    TransactionVariant::Hash(inner.hash)
                                }
                            }
                            TransactionVariant::Hash(_) => {
                                if full_transactions {
                                    panic!(
                                        "unexpected non full transaction for block {}",
                                        block_hash
                                    )
                                } else {
                                    transaction
                                }
                            }
                        })
                        .collect();

                    Ok(Some(block))
                }
                None => Ok(None),
            }
        })
    }

    /// Returns the code stored at the specified address.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to retrieve the code from.
    /// * `_block` - An optional block ID variant.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing the result of the operation, which is a `jsonrpc_core::Result` containing
    /// the code as a `Bytes` object.
    fn get_code(
        &self,
        address: zksync_basic_types::Address,
        _block: Option<BlockIdVariant>,
    ) -> RpcResult<Bytes> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let code_key = get_code_key(&address);

            match inner.write() {
                Ok(guard) => match guard.fork_storage.read_value_internal(&code_key) {
                    Ok(code_hash) => {
                        match guard.fork_storage.load_factory_dep_internal(code_hash) {
                            Ok(raw_code) => {
                                let code = raw_code.unwrap_or_default();
                                Ok(Bytes::from(code))
                            }
                            Err(error) => Err(report_into_jsrpc_error(error)),
                        }
                    }
                    Err(error) => Err(report_into_jsrpc_error(error)),
                },
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire write lock for code retrieval"),
                ))),
            }
        })
    }

    /// Returns the transaction count for a given address.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to get the transaction count for.
    /// * `_block` - Optional block ID variant.
    ///
    /// # Returns
    ///
    /// Returns a `BoxFuture` containing the transaction count as a `U256` wrapped in a `jsonrpc_core::Result`.
    fn get_transaction_count(
        &self,
        address: zksync_basic_types::Address,
        _block: Option<BlockIdVariant>,
    ) -> RpcResult<U256> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let nonce_key = get_nonce_key(&address);

            match inner.write() {
                Ok(guard) => match guard.fork_storage.read_value_internal(&nonce_key) {
                    Ok(result) => Ok(h256_to_u64(result).into()),
                    Err(error) => Err(report_into_jsrpc_error(error)),
                },
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire write lock for nonce retrieval"),
                ))),
            }
        })
    }

    /// Retrieves the transaction receipt for a given transaction hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - The hash of the transaction to retrieve the receipt for.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` that resolves to an `Option` of a `TransactionReceipt` or an error.
    fn get_transaction_receipt(
        &self,
        hash: zksync_basic_types::H256,
    ) -> RpcResult<Option<zksync_types::api::TransactionReceipt>> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let reader = match inner.read() {
                Ok(r) => r,
                Err(_) => {
                    return Err(into_jsrpc_error(Web3Error::InternalError(
                        anyhow::Error::msg(
                            "Failed to acquire read lock for transaction receipt retrieval",
                        ),
                    )))
                }
            };

            let receipt = reader
                .tx_results
                .get(&hash)
                .map(|info| info.receipt.clone());
            Ok(receipt)
        })
    }

    /// Sends a raw transaction to the L2 network.
    ///
    /// # Arguments
    ///
    /// * `tx_bytes` - The transaction bytes to send.
    ///
    /// # Returns
    ///
    /// A future that resolves to the hash of the transaction if successful, or an error if the transaction is invalid or execution fails.
    fn send_raw_transaction(&self, tx_bytes: Bytes) -> RpcResult<zksync_basic_types::H256> {
        let chain_id = match self.get_inner().read() {
            Ok(reader) => reader.fork_storage.chain_id,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire read lock for chain ID retrieval"),
                )))
                .boxed()
            }
        };

        let (tx_req, hash) = match TransactionRequest::from_bytes(&tx_bytes.0, chain_id) {
            Ok(result) => result,
            Err(e) => {
                return futures::future::err(into_jsrpc_error(Web3Error::SerializationError(e)))
                    .boxed()
            }
        };

        let mut l2_tx: L2Tx = match L2Tx::from_request(tx_req, MAX_TX_SIZE) {
            Ok(tx) => tx,
            Err(e) => {
                return futures::future::err(into_jsrpc_error(Web3Error::SerializationError(e)))
                    .boxed()
            }
        };

        l2_tx.set_input(tx_bytes.0, hash);
        if hash != l2_tx.hash() {
            let error_message =
                "Invalid transaction data: computed hash does not match the provided hash.";
            let web3_error = Web3Error::InternalError(anyhow::Error::msg(error_message));
            return futures::future::err(into_jsrpc_error(web3_error)).boxed();
        };

        match self.run_l2_tx(l2_tx.clone(), TxExecutionMode::VerifyExecute) {
            Ok(_) => Ok(hash).into_boxed_future(),
            Err(e) => {
                let error_message = format!("Execution error: {}", e);
                futures::future::err(into_jsrpc_error(Web3Error::SubmitTransactionError(
                    error_message,
                    l2_tx.hash().as_bytes().to_vec(),
                )))
                .boxed()
            }
        }
    }

    /// Returns a block by its hash. Currently, only hashes for blocks in memory are supported.
    ///
    /// # Arguments
    ///
    /// * `hash` - A `H256` type representing the hash of the block to retrieve.
    /// * `full_transactions` - A boolean value indicating whether to retrieve full transactions or not.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to an `Option` of `Block<TransactionVariant>`.
    fn get_block_by_hash(
        &self,
        hash: zksync_basic_types::H256,
        full_transactions: bool,
    ) -> RpcResult<Option<Block<TransactionVariant>>> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let maybe_block = {
                let reader = inner.read().map_err(|_| {
                    into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                        "Failed to acquire read lock for block retrieval.",
                    )))
                })?;

                // try retrieving block from memory, and if unavailable subsequently from the fork
                reader.blocks.get(&hash).cloned().or_else(|| {
                    reader
                        .fork_storage
                        .inner
                        .read()
                        .expect("failed reading fork storage")
                        .fork
                        .as_ref()
                        .and_then(|fork| {
                            fork.fork_source
                                .get_block_by_hash(hash, true)
                                .ok()
                                .flatten()
                        })
                })
            };

            match maybe_block {
                Some(mut block) => {
                    let block_hash = block.hash;
                    block.transactions = block
                        .transactions
                        .into_iter()
                        .map(|transaction| match &transaction {
                            TransactionVariant::Full(inner) => {
                                if full_transactions {
                                    transaction
                                } else {
                                    TransactionVariant::Hash(inner.hash)
                                }
                            }
                            TransactionVariant::Hash(_) => {
                                if full_transactions {
                                    panic!(
                                        "unexpected non full transaction for block {}",
                                        block_hash
                                    )
                                } else {
                                    transaction
                                }
                            }
                        })
                        .collect();

                    Ok(Some(block))
                }
                None => Ok(None),
            }
        })
    }

    /// Returns a future that resolves to an optional transaction with the given hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - A 32-byte hash of the transaction.
    ///
    /// # Returns
    ///
    /// A `jsonrpc_core::BoxFuture` that resolves to a `jsonrpc_core::Result` containing an optional `zksync_types::api::Transaction`.
    fn get_transaction_by_hash(
        &self,
        hash: zksync_basic_types::H256,
    ) -> RpcResult<Option<zksync_types::api::Transaction>> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let reader = inner.read().map_err(|_| {
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire read lock for transaction retrieval.",
                )))
            })?;

            let maybe_result = {
                // try retrieving transaction from memory, and if unavailable subsequently from the fork
                reader.tx_results.get(&hash).and_then(|TransactionResult { info, .. }| {
                    let input_data = info.tx.common_data.input.clone().or(None)?;
                    let chain_id = info.tx.common_data.extract_chain_id().or(None)?;
                    Some(zksync_types::api::Transaction {
                        hash,
                        nonce: U256::from(info.tx.common_data.nonce.0),
                        block_hash: Some(hash),
                        block_number: Some(U64::from(info.miniblock_number)),
                        transaction_index: Some(U64::from(0)),
                        from: Some(info.tx.initiator_account()),
                        to: Some(info.tx.recipient_account()),
                        value: info.tx.execute.value,
                        gas_price: Some(U256::from(0)),
                        gas: Default::default(),
                        input: input_data.data.into(),
                        v: Some(chain_id.into()),
                        r: Some(U256::zero()),
                        s: Some(U256::zero()),
                        raw: None,
                        transaction_type: {
                            let tx_type = match info.tx.common_data.transaction_type {
                                zksync_types::l2::TransactionType::LegacyTransaction => 0,
                                zksync_types::l2::TransactionType::EIP2930Transaction => 1,
                                zksync_types::l2::TransactionType::EIP1559Transaction => 2,
                                zksync_types::l2::TransactionType::EIP712Transaction => 113,
                                zksync_types::l2::TransactionType::PriorityOpTransaction => 255,
                                zksync_types::l2::TransactionType::ProtocolUpgradeTransaction => 254,
                            };
                            Some(tx_type.into())
                        },
                        access_list: None,
                        max_fee_per_gas: Some(info.tx.common_data.fee.max_fee_per_gas),
                        max_priority_fee_per_gas: Some(
                            info.tx.common_data.fee.max_priority_fee_per_gas,
                        ),
                        chain_id: U256::from(chain_id),
                        l1_batch_number: Some(U64::from(info.batch_number as u64)),
                        l1_batch_tx_index: None,
                    })
                }).or_else(|| {
                    reader
                        .fork_storage
                        .inner
                        .read()
                        .expect("failed reading fork storage")
                        .fork
                        .as_ref()
                        .and_then(|fork| {
                            fork.fork_source
                                .get_transaction_by_hash(hash)
                                .ok()
                                .flatten()
                        })
                })
            };

            Ok(maybe_result)
        })
    }

    /// Returns the current block number as a `U64` wrapped in a `BoxFuture`.
    fn get_block_number(&self) -> RpcResult<zksync_basic_types::U64> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let reader = inner.read().map_err(|_| {
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire read lock for block retrieval",
                )))
            })?;
            Ok(U64::from(reader.current_miniblock))
        })
    }

    /// Estimates the gas required for a given call request.
    ///
    /// # Arguments
    ///
    /// * `req` - A `CallRequest` struct representing the call request to estimate gas for.
    /// * `_block` - An optional `BlockNumber` struct representing the block number to estimate gas for.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `U256` representing the estimated gas required.
    fn estimate_gas(
        &self,
        req: zksync_types::transaction_request::CallRequest,
        _block: Option<BlockNumber>,
    ) -> RpcResult<U256> {
        let inner = self.get_inner().clone();
        let reader = match inner.read() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire read lock for gas estimation."),
                )))
                .boxed()
            }
        };

        let result: jsonrpc_core::Result<Fee> = reader.estimate_gas_impl(req);
        match result {
            Ok(fee) => Ok(fee.gas_limit).into_boxed_future(),
            Err(err) => return futures::future::err(err).boxed(),
        }
    }

    /// Returns the current gas price in U256 format.
    fn gas_price(&self) -> RpcResult<U256> {
        let fair_l2_gas_price: u64 = self
            .get_inner()
            .read()
            .expect("Failed to acquire read lock")
            .fee_input_provider
            .get_l2_gas_price();
        Ok(U256::from(fair_l2_gas_price)).into_boxed_future()
    }

    /// Creates a filter object, based on filter options, to notify when the state changes (logs).
    /// To check if the state has changed, call `eth_getFilterChanges`.
    ///
    /// # Arguments
    ///
    /// * `filter`: The filter options -
    ///     fromBlock: - Integer block number, or the string "latest", "earliest" or "pending".
    ///     toBlock: - Integer block number, or the string "latest", "earliest" or "pending".
    ///     address: - Contract address or a list of addresses from which the logs should originate.
    ///     topics: - [H256] topics. Topics are order-dependent. Each topic can also be an array with "or" options.
    ///
    /// If the from `fromBlock` or `toBlock` option are equal to "latest" the filter continually appends logs for newly mined blocks.
    /// Topics are order-dependent. A transaction with a log with topics [A, B] will be matched by the following topic filters:
    ///     * \[\] "anything"
    ///     * \[A\] "A in first position (and anything after)"
    ///     * \[null, B\] "anything in first position AND B in second position (and anything after)"
    ///     * \[A, B\] "A in first position AND B in second position (and anything after)"
    ///     * \[\[A, B\], \[A, B\]\] "(A OR B) in first position AND (A OR B) in second position (and anything after)"
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to an `U256` filter id.
    fn new_filter(&self, filter: Filter) -> RpcResult<U256> {
        let inner = self.get_inner().clone();
        let mut writer = match inner.write() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire write lock for filter creation"),
                )))
                .boxed()
            }
        };

        let from_block = filter.from_block.unwrap_or(BlockNumber::Latest);
        let to_block = filter.to_block.unwrap_or(BlockNumber::Latest);
        let addresses = filter.address.unwrap_or_default().0;
        let mut topics: [Option<HashSet<H256>>; 4] = Default::default();

        if let Some(filter_topics) = filter.topics {
            filter_topics
                .into_iter()
                .take(4)
                .enumerate()
                .for_each(|(i, maybe_topic_set)| {
                    if let Some(topic_set) = maybe_topic_set {
                        topics[i] = Some(topic_set.0.into_iter().collect());
                    }
                })
        }

        writer
            .filters
            .add_log_filter(from_block, to_block, addresses, topics)
            .map_err(|_| {
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire write lock for new filter.",
                )))
            })
            .into_boxed_future()
    }

    /// Creates a filter in the node, to notify when a new block arrives.
    /// To check if the state has changed, call `eth_getFilterChanges`.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to an `U256` filter id.
    fn new_block_filter(&self) -> RpcResult<U256> {
        let inner = self.get_inner().clone();
        let mut writer = match inner.write() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire write lock for new filter."),
                )))
                .boxed()
            }
        };

        writer
            .filters
            .add_block_filter()
            .map_err(|_| {
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire write lock for new block filter.",
                )))
            })
            .into_boxed_future()
    }

    /// Creates a filter in the node, to notify when new pending transactions arrive.
    /// To check if the state has changed, call `eth_getFilterChanges`.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to an `U256` filter id.
    fn new_pending_transaction_filter(&self) -> RpcResult<U256> {
        let inner = self.get_inner().clone();
        let mut writer = match inner.write() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire write lock for transaction filter."),
                )))
                .boxed()
            }
        };

        writer
            .filters
            .add_pending_transaction_filter()
            .map_err(|_| {
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire write lock for transaction filter.",
                )))
            })
            .into_boxed_future()
    }

    /// Uninstalls a filter with given id. Should always be called when watch is no longer needed.
    ///
    /// # Arguments
    ///
    /// * `id`: The filter id
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to an `U256` filter id.
    fn uninstall_filter(&self, id: U256) -> RpcResult<bool> {
        let inner = self.get_inner().clone();
        let mut writer = match inner.write() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire write lock for uninstalling filter."),
                )))
                .boxed()
            }
        };

        let result = writer.filters.remove_filter(id);
        Ok(result).into_boxed_future()
    }

    /// Returns an array of all logs matching a given filter.
    ///
    /// # Arguments
    ///
    /// * `filter`: The filter options -
    ///     fromBlock   - Integer block number, or the string "latest", "earliest" or "pending".
    ///     toBlock     - Integer block number, or the string "latest", "earliest" or "pending".
    ///     address     - Contract address or a list of addresses from which the logs should originate.
    ///     topics      - [H256] topics. Topics are order-dependent. Each topic can also be an array with "or" options.
    ///                   See `new_filter` documention for how to specify topics.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to an array of logs.
    fn get_logs(&self, filter: Filter) -> RpcResult<Vec<zksync_types::api::Log>> {
        let inner = self.get_inner();
        let reader = match inner.read() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire read lock for logs."),
                )))
                .boxed()
            }
        };
        let from_block = filter.from_block.unwrap_or(BlockNumber::Earliest);
        let to_block = filter.to_block.unwrap_or(BlockNumber::Latest);
        let addresses = filter.address.unwrap_or_default().0;
        let mut topics: [Option<HashSet<H256>>; 4] = Default::default();

        if let Some(filter_topics) = filter.topics {
            filter_topics
                .into_iter()
                .take(4)
                .enumerate()
                .for_each(|(i, maybe_topic_set)| {
                    if let Some(topic_set) = maybe_topic_set {
                        topics[i] = Some(topic_set.0.into_iter().collect());
                    }
                })
        }

        let log_filter = LogFilter::new(from_block, to_block, addresses, topics);

        let latest_block_number = U64::from(reader.current_miniblock);
        let logs = reader
            .tx_results
            .values()
            .flat_map(|tx_result| {
                tx_result
                    .receipt
                    .logs
                    .iter()
                    .filter(|log| log_filter.matches(log, latest_block_number))
                    .cloned()
            })
            .collect_vec();

        Ok(logs).into_boxed_future()
    }

    /// Returns an array of all logs matching filter with given id.
    ///
    /// # Arguments
    ///
    /// * `id`: The filter id
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to an array of logs.
    fn get_filter_logs(&self, id: U256) -> RpcResult<FilterChanges> {
        let inner = self.get_inner();
        let reader = match inner.read() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire read lock for get filter logs."),
                )))
                .boxed()
            }
        };

        let latest_block_number = U64::from(reader.current_miniblock);
        let logs = match reader.filters.get_filter(id) {
            Some(FilterType::Log(f)) => reader
                .tx_results
                .values()
                .flat_map(|tx_result| {
                    tx_result
                        .receipt
                        .logs
                        .iter()
                        .filter(|log| f.matches(log, latest_block_number))
                        .cloned()
                })
                .collect_vec(),
            _ => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire read lock for filter logs."),
                )))
                .boxed()
            }
        };

        Ok(FilterChanges::Logs(logs)).into_boxed_future()
    }

    /// Polling method for a filter, which returns an array of logs, block hashes, or transaction hashes,
    /// depending on the filter type, which occurred since last poll.
    ///
    /// # Arguments
    ///
    /// * `id`: The filter id
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to an array of logs, block hashes, or transaction hashes,
    /// depending on the filter type, which occurred since last poll.
    /// * Filters created with `eth_newFilter` return [zksync_types::api::Log] objects.
    /// * Filters created with `eth_newBlockFilter` return block hashes.
    /// * Filters created with `eth_newPendingTransactionFilter` return transaction hashes.
    fn get_filter_changes(&self, id: U256) -> RpcResult<FilterChanges> {
        let inner = self.get_inner().clone();
        let mut writer = match inner.write() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire write lock for filtered changes"),
                )))
                .boxed()
            }
        };

        writer
            .filters
            .get_new_changes(id)
            .map_err(|_| {
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire write lock for filtered changes.",
                )))
            })
            .into_boxed_future()
    }

    fn get_block_transaction_count_by_number(
        &self,
        block_number: BlockNumber,
    ) -> RpcResult<Option<U256>> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let maybe_result = {
                let reader = match inner.read() {
                    Ok(r) => r,
                    Err(_) => {
                        return Err(into_jsrpc_error(Web3Error::InternalError(
                            anyhow::Error::msg(
                                "Failed to acquire read lock for transaction count retrieval.",
                            ),
                        )))
                    }
                };
                let number =
                    utils::to_real_block_number(block_number, U64::from(reader.current_miniblock))
                        .as_u64();

                reader
                    .block_hashes
                    .get(&number)
                    .and_then(|hash| reader.blocks.get(hash))
                    .map(|block| U256::from(block.transactions.len()))
                    .or_else(|| {
                        reader
                            .fork_storage
                            .inner
                            .read()
                            .expect("failed reading fork storage")
                            .fork
                            .as_ref()
                            .and_then(|fork| {
                                fork.fork_source
                                    .get_block_transaction_count_by_number(block_number)
                                    .ok()
                                    .flatten()
                            })
                    })
            };

            match maybe_result {
                Some(value) => Ok(Some(value)),
                None => Err(into_jsrpc_error(Web3Error::NoBlock)),
            }
        })
    }

    fn get_block_transaction_count_by_hash(
        &self,
        block_hash: zksync_basic_types::H256,
    ) -> RpcResult<Option<U256>> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let reader = inner.read().map_err(|_| {
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire read lock for transaction count retrieval.",
                )))
            })?;

            // try retrieving block from memory, and if unavailable subsequently from the fork
            let maybe_result = reader
                .blocks
                .get(&block_hash)
                .map(|block| U256::from(block.transactions.len()))
                .or_else(|| {
                    reader
                        .fork_storage
                        .inner
                        .read()
                        .expect("failed reading fork storage")
                        .fork
                        .as_ref()
                        .and_then(|fork| {
                            fork.fork_source
                                .get_block_transaction_count_by_hash(block_hash)
                                .ok()
                                .flatten()
                        })
                });

            match maybe_result {
                Some(value) => Ok(Some(value)),
                None => Err(into_jsrpc_error(Web3Error::NoBlock)),
            }
        })
    }

    /// Returns the value from a storage position at a given address.
    ///
    /// # Arguments
    ///
    /// * `address`: Address of the storage
    /// * `idx`: Integer of the position in the storage
    /// * `block`: The block storage to target
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to a [H256] value in the storage.
    fn get_storage(
        &self,
        address: zksync_basic_types::Address,
        idx: U256,
        block: Option<BlockIdVariant>,
    ) -> RpcResult<zksync_basic_types::H256> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let writer = match inner.write() {
                Ok(r) => r,
                Err(_) => {
                    return Err(into_jsrpc_error(Web3Error::InternalError(
                        anyhow::Error::msg("Failed to acquire write lock for storage retrieval."),
                    )));
                }
            };

            let storage_key = StorageKey::new(AccountTreeId::new(address), u256_to_h256(idx));

            let block_number = block
                .map(|block| match block {
                    BlockIdVariant::BlockNumber(block_number) => Ok(utils::to_real_block_number(
                        block_number,
                        U64::from(writer.current_miniblock),
                    )),
                    BlockIdVariant::BlockNumberObject(o) => Ok(utils::to_real_block_number(
                        o.block_number,
                        U64::from(writer.current_miniblock),
                    )),
                    BlockIdVariant::BlockHashObject(o) => writer
                        .blocks
                        .get(&o.block_hash)
                        .map(|block| block.number)
                        .ok_or_else(|| {
                            tracing::error!(
                                "unable to map block number to hash #{:#x}",
                                o.block_hash
                            );
                            into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                                "Failed to map block number to hash.",
                            )))
                        }),
                })
                .unwrap_or_else(|| Ok(U64::from(writer.current_miniblock)))?;

            if block_number.as_u64() == writer.current_miniblock {
                match writer.fork_storage.read_value_internal(&storage_key) {
                    Ok(value) => Ok(H256(value.0)),
                    Err(error) => Err(report_into_jsrpc_error(error)),
                }
            } else if writer.block_hashes.contains_key(&block_number.as_u64()) {
                let value = writer
                    .block_hashes
                    .get(&block_number.as_u64())
                    .and_then(|block_hash| writer.previous_states.get(block_hash))
                    .and_then(|state| state.get(&storage_key))
                    .cloned()
                    .unwrap_or_default();

                if value.is_zero() {
                    match writer.fork_storage.read_value_internal(&storage_key) {
                        Ok(value) => Ok(H256(value.0)),
                        Err(error) => Err(report_into_jsrpc_error(error)),
                    }
                } else {
                    Ok(value)
                }
            } else {
                writer
                    .fork_storage
                    .inner
                    .read()
                    .expect("failed reading fork storage")
                    .fork
                    .as_ref()
                    .and_then(|fork| fork.fork_source.get_storage_at(address, idx, block).ok())
                    .ok_or_else(|| {
                        tracing::error!(
                            "unable to get storage at address {:?}, index {:?} for block {:?}",
                            address,
                            idx,
                            block
                        );
                        into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                            "Failed to get storage.",
                        )))
                    })
            }
        })
    }

    /// Returns information about a transaction by block hash and transaction index position.
    ///
    /// # Arguments
    ///
    /// * `block_hash`: Hash of a block
    /// * `index`: Integer of the transaction index position
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that maybe resolves to a [zksync_types::api::Transaction], if found.
    fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: zksync_basic_types::H256,
        index: zksync_basic_types::web3::Index,
    ) -> RpcResult<Option<zksync_types::api::Transaction>> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let reader = match inner.read() {
                Ok(r) => r,
                Err(_) => {
                    return Err(into_jsrpc_error(Web3Error::InternalError(
                        anyhow::Error::msg("Failed to get storage."),
                    )));
                }
            };

            let maybe_tx = reader
                .blocks
                .get(&block_hash)
                .and_then(|block| block.transactions.get(index.as_usize()))
                .and_then(|tx| match tx {
                    TransactionVariant::Full(tx) => Some(tx.clone()),
                    TransactionVariant::Hash(tx_hash) => reader
                        .fork_storage
                        .inner
                        .read()
                        .expect("failed reading fork storage")
                        .fork
                        .as_ref()
                        .and_then(|fork| {
                            fork.fork_source
                                .get_transaction_by_hash(*tx_hash)
                                .ok()
                                .flatten()
                        }),
                })
                .or_else(|| {
                    reader
                        .fork_storage
                        .inner
                        .read()
                        .expect("failed reading fork storage")
                        .fork
                        .as_ref()
                        .and_then(|fork| {
                            fork.fork_source
                                .get_transaction_by_block_hash_and_index(block_hash, index)
                                .ok()
                        })
                        .flatten()
                });

            Ok(maybe_tx)
        })
    }

    /// Returns information about a transaction by block number and transaction index position.
    ///
    /// # Arguments
    ///
    /// * `block_number`: A block number, or the string "earliest", "latest" or "pending".
    /// * `index`: Integer of the transaction index position
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that maybe resolves to a [zksync_types::api::Transaction], if found.
    fn get_transaction_by_block_number_and_index(
        &self,
        block_number: BlockNumber,
        index: zksync_basic_types::web3::Index,
    ) -> RpcResult<Option<zksync_types::api::Transaction>> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let reader = match inner.read() {
                Ok(r) => r,
                Err(_) => {
                    return Err(into_jsrpc_error(Web3Error::InternalError(
                        anyhow::Error::msg(
                            "Failed to acquire read lock for transaction retrieval.",
                        ),
                    )));
                }
            };

            let real_block_number =
                utils::to_real_block_number(block_number, U64::from(reader.current_miniblock));
            let maybe_tx = reader
                .block_hashes
                .get(&real_block_number.as_u64())
                .and_then(|block_hash| reader.blocks.get(block_hash))
                .and_then(|block| block.transactions.get(index.as_usize()))
                .and_then(|tx| match tx {
                    TransactionVariant::Full(tx) => Some(tx.clone()),
                    TransactionVariant::Hash(tx_hash) => reader
                        .fork_storage
                        .inner
                        .read()
                        .expect("failed reading fork storage")
                        .fork
                        .as_ref()
                        .and_then(|fork| {
                            fork.fork_source
                                .get_transaction_by_hash(*tx_hash)
                                .ok()
                                .flatten()
                        }),
                })
                .or_else(|| {
                    reader
                        .fork_storage
                        .inner
                        .read()
                        .expect("failed reading fork storage")
                        .fork
                        .as_ref()
                        .and_then(|fork| {
                            fork.fork_source
                                .get_transaction_by_block_number_and_index(block_number, index)
                                .ok()
                        })
                        .flatten()
                });

            Ok(maybe_tx)
        })
    }

    /// Returns the protocol version.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to a hex `String` of the version number.
    fn protocol_version(&self) -> RpcResult<String> {
        Ok(String::from(PROTOCOL_VERSION)).into_boxed_future()
    }

    fn syncing(&self) -> RpcResult<SyncState> {
        Ok(SyncState::NotSyncing).into_boxed_future()
    }
    /// Returns a list of available accounts.
    ///
    /// This function fetches the accounts from the inner state, and returns them as a list of addresses (`H160`).
    ///
    /// # Errors
    ///
    /// Returns a `jsonrpc_core::Result` error if acquiring a write lock on the inner state fails.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `jsonrpc_core::Result` that resolves to a `Vec<H160>` of addresses.
    fn accounts(&self) -> RpcResult<Vec<H160>> {
        let inner = self.get_inner().clone();
        let reader = match inner.read() {
            Ok(r) => r,
            Err(_) => {
                return futures::future::err(into_jsrpc_error(Web3Error::InternalError(
                    anyhow::Error::msg("Failed to acquire read lock for account retrieval."),
                )))
                .boxed()
            }
        };

        let accounts: Vec<H160> = reader.rich_accounts.clone().into_iter().collect();
        futures::future::ok(accounts).boxed()
    }

    fn coinbase(&self) -> RpcResult<zksync_basic_types::Address> {
        not_implemented("eth_coinbase")
    }

    fn compilers(&self) -> RpcResult<Vec<String>> {
        not_implemented("eth_getCompilers")
    }

    fn hashrate(&self) -> RpcResult<U256> {
        not_implemented("eth_hashrate")
    }

    fn get_uncle_count_by_block_hash(
        &self,
        _hash: zksync_basic_types::H256,
    ) -> RpcResult<Option<U256>> {
        not_implemented("eth_getUncleCountByBlockHash")
    }

    fn get_uncle_count_by_block_number(&self, _number: BlockNumber) -> RpcResult<Option<U256>> {
        not_implemented("eth_getUncleCountByBlockNumber")
    }

    fn mining(&self) -> RpcResult<bool> {
        not_implemented("eth_mining")
    }

    /// Returns the fee history for a given range of blocks.
    ///
    /// Note: This implementation is limited to using the hard-coded value
    /// of L2_GAS_PRICE as the history gas price
    ///
    /// # Arguments
    ///
    /// * `block_count` - The number of blocks in the requested range. Between 1 and 1024 blocks can be requested in a single query. It will return less than the requested range if not all blocks are available.
    /// * `newest_block` - The highest number block of the requested range. As this implementation is using hard-coded values, this argument is ignored.
    /// * `reward_percentiles` - A list of percentile values with a monotonic increase in value.
    ///
    /// # Returns
    ///
    /// A `BoxFuture` containing a `Result` with a `FeeHistory` representing the fee history of the specified range of blocks.
    fn fee_history(
        &self,
        block_count: U64,
        _newest_block: BlockNumber,
        reward_percentiles: Vec<f32>,
    ) -> RpcResult<FeeHistory> {
        let inner = self.get_inner().clone();

        Box::pin(async move {
            let reader = inner.read().map_err(|_| {
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire read lock for fee history.",
                )))
            })?;

            let block_count = block_count
                .as_u64()
                .min(1024)
                // Can't be more than the total number of blocks
                .clamp(1, reader.current_miniblock + 1);

            let mut base_fee_per_gas = vec![
                U256::from(reader.fee_input_provider.get_l2_gas_price());
                block_count as usize
            ];

            let oldest_block = reader.current_miniblock + 1 - base_fee_per_gas.len() as u64;
            // We do not store gas used ratio for blocks, returns array of zeroes as a placeholder.
            let gas_used_ratio = vec![0.0; base_fee_per_gas.len()];
            // Effective priority gas price is currently 0.
            let reward = Some(vec![
                vec![U256::zero(); reward_percentiles.len()];
                base_fee_per_gas.len()
            ]);

            // `base_fee_per_gas` for next miniblock cannot be calculated, appending last fee as a placeholder.
            base_fee_per_gas.push(*base_fee_per_gas.last().unwrap());

            Ok(FeeHistory {
                oldest_block: web3::BlockNumber::Number(oldest_block.into()),
                base_fee_per_gas,
                gas_used_ratio,
                reward,
            })
        })
    }
}

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> EthTestNodeNamespaceT
    for InMemoryNode<S>
{
    /// Sends a transaction to the L2 network. Can be used for the impersonated account.
    ///
    /// # Arguments
    ///
    /// * `tx` - A `CallRequest` struct representing the transaction.
    ///
    /// # Returns
    ///
    /// A future that resolves to the hash of the transaction if successful, or an error if the transaction is invalid or execution fails.
    fn send_transaction(
        &self,
        tx: zksync_types::transaction_request::CallRequest,
    ) -> jsonrpc_core::BoxFuture<jsonrpc_core::Result<zksync_basic_types::H256>> {
        let l1_gas_price = match self.get_inner().read() {
            Ok(reader) => reader.fee_input_provider.get_l1_gas_price(),
            Err(_) => {
                return futures::future::err(into_jsrpc_error_message(
                    "Failed to acquire read lock for l1 gas price retrieval.".to_string(),
                ))
                .boxed()
            }
        };

        let mut tx_req = TransactionRequest::from(tx.clone());
        // Users might expect a "sensible default"
        if tx.gas.is_none() {
            tx_req.gas = U256::from(MAX_L1_TRANSACTION_GAS_LIMIT);
        }

        // EIP-1559 gas fields should be processed separately
        if tx.gas_price.is_some() {
            if tx.max_fee_per_gas.is_some() || tx.max_priority_fee_per_gas.is_some() {
                let error_message = "Transaction contains unsupported fields: max_fee_per_gas or max_priority_fee_per_gas";
                tracing::error!("{}", error_message);
                return futures::future::err(into_jsrpc_error_message(error_message.to_string()))
                    .boxed();
            }
        } else {
            tx_req.gas_price = tx.max_fee_per_gas.unwrap_or(U256::from(l1_gas_price));
            tx_req.max_priority_fee_per_gas = tx.max_priority_fee_per_gas;
            if tx_req.transaction_type.is_none() {
                tx_req.transaction_type = Some(zksync_types::EIP_1559_TX_TYPE.into());
            }
        }
        // Needed to calculate hash
        tx_req.r = Some(U256::default());
        tx_req.s = Some(U256::default());
        tx_req.v = Some(U64::from(27));

        let hash = match tx_req.get_tx_hash() {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("Transaction request serialization error: {}", e);
                return futures::future::err(into_jsrpc_error(Web3Error::SerializationError(e)))
                    .boxed();
            }
        };
        let bytes = match tx_req.get_signed_bytes(&PackedEthSignature::from_rsv(
            &H256::default(),
            &H256::default(),
            27,
        )) {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("Transaction request serialization error: {}", e);
                return futures::future::err(into_jsrpc_error(Web3Error::SerializationError(e)))
                    .boxed();
            }
        };
        let mut l2_tx: L2Tx = match L2Tx::from_request(tx_req, MAX_TX_SIZE) {
            Ok(tx) => tx,
            Err(e) => {
                tracing::error!("Transaction serialization error: {}", e);
                return futures::future::err(into_jsrpc_error(Web3Error::SerializationError(e)))
                    .boxed();
            }
        };

        // `v` was overwritten with 0 during converting into l2 tx
        let mut signature = vec![0u8; 65];
        signature[64] = 27;
        l2_tx.common_data.signature = signature;

        l2_tx.set_input(bytes, hash);

        match self.get_inner().read() {
            Ok(reader) => {
                if !reader
                    .impersonated_accounts
                    .contains(&l2_tx.common_data.initiator_address)
                {
                    let error_message = format!(
                        "Initiator address {:?} is not allowed to perform transactions",
                        l2_tx.common_data.initiator_address
                    );
                    tracing::error!("{}", error_message);
                    return futures::future::err(into_jsrpc_error_message(error_message)).boxed();
                }
            }
            Err(_) => {
                return futures::future::err(into_jsrpc_error_message(
                    "Failed to acquire read lock for accounts.".to_string(),
                ))
                .boxed()
            }
        }

        match self.run_l2_tx(l2_tx.clone(), TxExecutionMode::VerifyExecute) {
            Ok(_) => Ok(l2_tx.hash()).into_boxed_future(),
            Err(e) => {
                let error_message = format!("Execution error: {}", e);
                futures::future::err(into_jsrpc_error_message(error_message)).boxed()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{cache::CacheConfig, gas::DEFAULT_L2_GAS_PRICE},
        fork::ForkDetails,
        http_fork_source::HttpForkSource,
        node::{compute_hash, InMemoryNode, Snapshot},
        testing::{
            self, default_tx_debug_info, ForkBlockConfig, LogBuilder, MockServer,
            TransactionResponseBuilder,
        },
    };
    use maplit::hashmap;
    use zksync_basic_types::{web3, Nonce};
    use zksync_types::{
        api::{BlockHashObject, BlockNumber, BlockNumberObject, TransactionReceipt},
        utils::deployed_address_create,
        K256PrivateKey,
    };
    use zksync_web3_decl::types::{SyncState, ValueOrArray};

    use super::*;

    #[tokio::test]
    async fn test_eth_syncing() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let syncing = node.syncing().await.expect("failed syncing");
        assert!(matches!(syncing, SyncState::NotSyncing));
    }

    #[tokio::test]
    async fn test_get_fee_history_with_1_block() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let fee_history = node
            .fee_history(U64::from(1), BlockNumber::Latest, vec![25.0, 50.0, 75.0])
            .await
            .expect("fee_history failed");

        assert_eq!(
            fee_history.oldest_block,
            web3::BlockNumber::Number(U64::from(0))
        );
        assert_eq!(
            fee_history.base_fee_per_gas,
            vec![U256::from(DEFAULT_L2_GAS_PRICE); 2]
        );
        assert_eq!(fee_history.gas_used_ratio, vec![0.0]);
        assert_eq!(fee_history.reward, Some(vec![vec![U256::from(0); 3]]));
    }

    #[tokio::test]
    async fn test_get_fee_history_with_no_reward_percentiles() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let fee_history = node
            .fee_history(U64::from(1), BlockNumber::Latest, vec![])
            .await
            .expect("fee_history failed");

        assert_eq!(
            fee_history.oldest_block,
            web3::BlockNumber::Number(U64::from(0))
        );
        assert_eq!(
            fee_history.base_fee_per_gas,
            vec![U256::from(DEFAULT_L2_GAS_PRICE); 2]
        );
        assert_eq!(fee_history.gas_used_ratio, vec![0.0]);
        assert_eq!(fee_history.reward, Some(vec![vec![]]));
    }

    #[tokio::test]
    async fn test_get_fee_history_with_multiple_blocks() {
        // Arrange
        let node = InMemoryNode::<HttpForkSource>::default();
        testing::apply_tx(&node, H256::repeat_byte(0x01));

        // Act
        let latest_block = node
            .get_block_number()
            .await
            .expect("Block number fetch failed");
        let fee_history = node
            .fee_history(U64::from(2), BlockNumber::Latest, vec![25.0, 50.0, 75.0])
            .await
            .expect("fee_history failed");

        // Assert
        // We should receive 2 fees: from block 1 and 2.
        assert_eq!(latest_block, U64::from(2));
        assert_eq!(
            fee_history.oldest_block,
            web3::BlockNumber::Number(U64::from(1))
        );
        assert_eq!(
            fee_history.base_fee_per_gas,
            vec![U256::from(DEFAULT_L2_GAS_PRICE); 3]
        );
        assert_eq!(fee_history.gas_used_ratio, vec![0.0, 0.0]);
        assert_eq!(fee_history.reward, Some(vec![vec![U256::from(0); 3]; 2]));
    }

    #[tokio::test]
    async fn test_get_block_by_hash_returns_none_for_non_existing_block() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let result = node
            .get_block_by_hash(H256::repeat_byte(0x01), false)
            .await
            .expect("failed fetching block by hash");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_node_has_genesis_block() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let block = node
            .get_block_by_number(BlockNumber::Latest, false)
            .await
            .expect("failed fetching block by number")
            .expect("no block");

        assert_eq!(0, block.number.as_u64());
        assert_eq!(compute_hash(0, H256::zero()), block.hash);
    }

    #[tokio::test]
    async fn test_node_creates_genesis_block_with_hash_and_zero_parent_hash() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let block = node
            .get_block_by_hash(compute_hash(0, H256::zero()), false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(block.parent_hash, H256::zero());
    }

    #[tokio::test]
    async fn test_node_produces_blocks_with_parent_hash_links() {
        let node = InMemoryNode::<HttpForkSource>::default();
        testing::apply_tx(&node, H256::repeat_byte(0x01));

        let genesis_block = node
            .get_block_by_number(BlockNumber::from(0), false)
            .await
            .expect("failed fetching block by number")
            .expect("no block");
        let first_block = node
            .get_block_by_number(BlockNumber::from(1), false)
            .await
            .expect("failed fetching block by number")
            .expect("no block");
        let second_block = node
            .get_block_by_number(BlockNumber::from(2), false)
            .await
            .expect("failed fetching block by number")
            .expect("no block");

        assert_eq!(genesis_block.hash, first_block.parent_hash);
        assert_eq!(first_block.hash, second_block.parent_hash);
    }

    #[tokio::test]
    async fn test_get_block_by_hash_for_produced_block() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let (expected_block_hash, _) = testing::apply_tx(&node, H256::repeat_byte(0x01));

        let actual_block = node
            .get_block_by_hash(expected_block_hash, false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(expected_block_hash, actual_block.hash);
        assert_eq!(U64::from(1), actual_block.number);
        assert_eq!(Some(U64::from(1)), actual_block.l1_batch_number);
    }

    #[tokio::test]
    async fn test_node_block_mapping_is_correctly_populated_when_using_fork_source() {
        let input_block_number = 8;
        let input_block_hash = H256::repeat_byte(0x01);
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: input_block_number,
            hash: input_block_hash,
            transaction_count: 0,
        });

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        let inner = node.get_inner();
        let inner = inner.read().unwrap();
        assert!(
            inner.blocks.contains_key(&input_block_hash),
            "block wasn't cached"
        );
        assert!(
            inner.block_hashes.contains_key(&input_block_number),
            "block number wasn't cached"
        );
    }

    #[tokio::test]
    async fn test_get_block_by_hash_uses_fork_source() {
        let input_block_hash = H256::repeat_byte(0x01);

        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let mock_block_number = 8;
        let block_response = testing::BlockResponseBuilder::new()
            .set_hash(input_block_hash)
            .set_number(mock_block_number)
            .build();
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getBlockByHash",
                "params": [
                    format!("{input_block_hash:#x}"),
                    true
                ],
            }),
            block_response,
        );
        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        let actual_block = node
            .get_block_by_hash(input_block_hash, false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(input_block_hash, actual_block.hash);
        assert_eq!(U64::from(mock_block_number), actual_block.number);
        assert_eq!(Some(U64::from(6)), actual_block.l1_batch_number);
    }

    #[tokio::test]
    async fn test_get_block_by_number_returns_none_for_non_existing_block() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let result = node
            .get_block_by_number(BlockNumber::Number(U64::from(42)), false)
            .await
            .expect("failed fetching block by number");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_block_by_number_for_produced_block() {
        let node = InMemoryNode::<HttpForkSource>::default();
        testing::apply_tx(&node, H256::repeat_byte(0x01));
        let expected_block_number = 1;

        let actual_block = node
            .get_block_by_number(BlockNumber::Number(U64::from(expected_block_number)), false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(U64::from(expected_block_number), actual_block.number);
        assert_eq!(1, actual_block.transactions.len());
    }

    #[tokio::test]
    async fn test_get_block_by_number_uses_fork_source_if_missing_number() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let mock_block_number = 8;
        let block_response = testing::BlockResponseBuilder::new()
            .set_number(mock_block_number)
            .build();
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getBlockByNumber",
                "params": [
                    "0x8",
                    true
                ],
            }),
            block_response,
        );
        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        let actual_block = node
            .get_block_by_number(BlockNumber::Number(U64::from(8)), false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");
        assert_eq!(U64::from(mock_block_number), actual_block.number);
    }

    #[tokio::test]
    async fn test_get_block_by_number_for_latest_block_produced_locally() {
        let node = InMemoryNode::<HttpForkSource>::default();
        testing::apply_tx(&node, H256::repeat_byte(0x01));

        // The latest block, will be the 'virtual' one with 0 transactions (block 2).
        let virtual_block = node
            .get_block_by_number(BlockNumber::Latest, true)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(U64::from(2), virtual_block.number);
        assert_eq!(0, virtual_block.transactions.len());

        let actual_block = node
            .get_block_by_number(BlockNumber::Number(U64::from(1)), true)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");

        assert_eq!(U64::from(1), actual_block.number);
        assert_eq!(1, actual_block.transactions.len());
    }

    #[tokio::test]
    async fn test_get_block_by_number_uses_locally_available_block_for_latest_block() {
        let input_block_number = 10;
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: input_block_number,
            hash: H256::repeat_byte(0x01),
            transaction_count: 0,
        });

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        let actual_block = node
            .get_block_by_number(BlockNumber::Latest, false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");
        assert_eq!(U64::from(input_block_number), actual_block.number);
    }

    #[tokio::test]
    async fn test_get_block_by_number_uses_fork_source_for_earliest_block() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let input_block_number = 1;
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getBlockByNumber",
                "params": [
                    "earliest",
                    true
                ],
            }),
            testing::BlockResponseBuilder::new()
                .set_number(input_block_number)
                .build(),
        );
        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        let actual_block = node
            .get_block_by_number(BlockNumber::Earliest, false)
            .await
            .expect("failed fetching block by hash")
            .expect("no block");
        assert_eq!(U64::from(input_block_number), actual_block.number);
    }

    #[tokio::test]
    async fn test_get_block_by_number_uses_locally_available_for_latest_alike_blocks() {
        for block_number in [
            BlockNumber::Pending,
            BlockNumber::Committed,
            BlockNumber::Finalized,
        ] {
            let input_block_number = 10;
            let mock_server = MockServer::run_with_config(ForkBlockConfig {
                number: input_block_number,
                hash: H256::repeat_byte(0xab),
                transaction_count: 0,
            });
            let node = InMemoryNode::<HttpForkSource>::new(
                Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
                None,
                Default::default(),
                Default::default(),
            );

            let actual_block = node
                .get_block_by_number(block_number, false)
                .await
                .expect("failed fetching block by hash")
                .expect("no block");
            assert_eq!(
                U64::from(input_block_number),
                actual_block.number,
                "case {}",
                block_number,
            );
        }
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_hash_for_produced_block() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let (expected_block_hash, _) = testing::apply_tx(&node, H256::repeat_byte(0x01));
        let actual_transaction_count = node
            .get_block_transaction_count_by_hash(expected_block_hash)
            .await
            .expect("failed fetching block by hash")
            .expect("no result");

        assert_eq!(U256::from(1), actual_transaction_count);
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_hash_uses_fork_source() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let input_block_hash = H256::repeat_byte(0x01);
        let input_transaction_count = 1;
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getBlockTransactionCountByHash",
                "params": [
                    format!("{:#x}", input_block_hash),
                ],
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "result": format!("{:#x}", input_transaction_count),
            }),
        );
        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        let actual_transaction_count = node
            .get_block_transaction_count_by_hash(input_block_hash)
            .await
            .expect("failed fetching block by hash")
            .expect("no result");

        assert_eq!(
            U256::from(input_transaction_count),
            actual_transaction_count
        );
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_number_for_produced_block() {
        let node = InMemoryNode::<HttpForkSource>::default();

        testing::apply_tx(&node, H256::repeat_byte(0x01));
        let actual_transaction_count = node
            .get_block_transaction_count_by_number(BlockNumber::Number(U64::from(1)))
            .await
            .expect("failed fetching block by hash")
            .expect("no result");

        assert_eq!(U256::from(1), actual_transaction_count);
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_number_uses_fork_source() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let input_block_number = 1;
        let input_transaction_count = 1;
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getBlockTransactionCountByNumber",
                "params": [
                    format!("{:#x}", input_block_number),
                ],
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "result": format!("{:#x}", input_transaction_count),
            }),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        let actual_transaction_count = node
            .get_block_transaction_count_by_number(BlockNumber::Number(U64::from(1)))
            .await
            .expect("failed fetching block by hash")
            .expect("no result");

        assert_eq!(
            U256::from(input_transaction_count),
            actual_transaction_count
        );
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_number_earliest_uses_fork_source() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            hash: H256::repeat_byte(0xab),
            transaction_count: 0,
        });
        let input_transaction_count = 1;
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getBlockTransactionCountByNumber",
                "params": [
                    "earliest",
                ],
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "result": format!("{:#x}", input_transaction_count),
            }),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        let actual_transaction_count = node
            .get_block_transaction_count_by_number(BlockNumber::Earliest)
            .await
            .expect("failed fetching block by hash")
            .expect("no result");

        assert_eq!(
            U256::from(input_transaction_count),
            actual_transaction_count
        );
    }

    #[tokio::test]
    async fn test_get_block_transaction_count_by_number_latest_alike_uses_fork_source() {
        for block_number in [
            BlockNumber::Latest,
            BlockNumber::Pending,
            BlockNumber::Committed,
            BlockNumber::Finalized,
        ] {
            let input_transaction_count = 1;
            let mock_server = MockServer::run_with_config(ForkBlockConfig {
                number: 10,
                transaction_count: input_transaction_count,
                hash: H256::repeat_byte(0xab),
            });

            let node = InMemoryNode::<HttpForkSource>::new(
                Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
                None,
                Default::default(),
                Default::default(),
            );

            let actual_transaction_count = node
                .get_block_transaction_count_by_number(block_number)
                .await
                .expect("failed fetching block by hash")
                .expect("no result");

            assert_eq!(
                U256::from(input_transaction_count),
                actual_transaction_count,
                "case {}",
                block_number,
            );
        }
    }

    #[tokio::test]
    async fn test_get_transaction_receipt_uses_produced_block_hash() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let tx_hash = H256::repeat_byte(0x01);
        let (expected_block_hash, _) = testing::apply_tx(&node, tx_hash);

        let actual_tx_receipt = node
            .get_transaction_receipt(tx_hash)
            .await
            .expect("failed fetching transaction receipt by hash")
            .expect("no transaction receipt");

        assert_eq!(expected_block_hash, actual_tx_receipt.block_hash);
    }

    #[tokio::test]
    async fn test_new_block_filter_returns_filter_id() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let actual_filter_id = node
            .new_block_filter()
            .await
            .expect("failed creating filter");

        assert_eq!(U256::from(1), actual_filter_id);
    }

    #[tokio::test]
    async fn test_new_filter_returns_filter_id() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let actual_filter_id = node
            .new_filter(Filter::default())
            .await
            .expect("failed creating filter");

        assert_eq!(U256::from(1), actual_filter_id);
    }

    #[tokio::test]
    async fn test_new_pending_transaction_filter_returns_filter_id() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let actual_filter_id = node
            .new_pending_transaction_filter()
            .await
            .expect("failed creating filter");

        assert_eq!(U256::from(1), actual_filter_id);
    }

    #[tokio::test]
    async fn test_uninstall_filter_returns_true_if_filter_exists() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let filter_id = node
            .new_block_filter()
            .await
            .expect("failed creating filter");

        let actual_result = node
            .uninstall_filter(filter_id)
            .await
            .expect("failed creating filter");

        assert!(actual_result);
    }

    #[tokio::test]
    async fn test_uninstall_filter_returns_false_if_filter_does_not_exist() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let actual_result = node
            .uninstall_filter(U256::from(100))
            .await
            .expect("failed creating filter");

        assert!(!actual_result);
    }

    #[tokio::test]
    async fn test_get_filter_changes_returns_block_hash_updates_only_once() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let filter_id = node
            .new_block_filter()
            .await
            .expect("failed creating filter");
        let (block_hash, _) = testing::apply_tx(&node, H256::repeat_byte(0x1));

        match node
            .get_filter_changes(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Hashes(result) => {
                // Get the block hash and the virtual block hash.
                assert_eq!(2, result.len());
                assert_eq!(block_hash, result[0]);
            }
            changes => panic!("unexpected filter changes: {:?}", changes),
        }

        match node
            .get_filter_changes(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Empty(_) => (),
            changes => panic!("expected no changes in the second call, got {:?}", changes),
        }
    }

    #[tokio::test]
    async fn test_get_filter_changes_returns_log_updates_only_once() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let filter_id = node
            .new_filter(Filter {
                from_block: None,
                to_block: None,
                address: None,
                topics: None,
                block_hash: None,
            })
            .await
            .expect("failed creating filter");
        testing::apply_tx(&node, H256::repeat_byte(0x1));

        match node
            .get_filter_changes(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Logs(result) => assert_eq!(3, result.len()),
            changes => panic!("unexpected filter changes: {:?}", changes),
        }

        match node
            .get_filter_changes(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Empty(_) => (),
            changes => panic!("expected no changes in the second call, got {:?}", changes),
        }
    }

    #[tokio::test]
    async fn test_get_filter_changes_returns_pending_transaction_updates_only_once() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let filter_id = node
            .new_pending_transaction_filter()
            .await
            .expect("failed creating filter");
        testing::apply_tx(&node, H256::repeat_byte(0x1));

        match node
            .get_filter_changes(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Hashes(result) => assert_eq!(vec![H256::repeat_byte(0x1)], result),
            changes => panic!("unexpected filter changes: {:?}", changes),
        }

        match node
            .get_filter_changes(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Empty(_) => (),
            changes => panic!("expected no changes in the second call, got {:?}", changes),
        }
    }

    #[tokio::test]
    async fn test_produced_block_archives_previous_blocks() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let input_storage_key = StorageKey::new(
            AccountTreeId::new(H160::repeat_byte(0x1)),
            u256_to_h256(U256::zero()),
        );
        let input_storage_value = H256::repeat_byte(0xcd);
        node.get_inner()
            .write()
            .unwrap()
            .fork_storage
            .set_value(input_storage_key, input_storage_value);
        let initial_miniblock = node.get_inner().read().unwrap().current_miniblock;

        testing::apply_tx(&node, H256::repeat_byte(0x1));
        let current_miniblock = node.get_inner().read().unwrap().current_miniblock;

        let inner = node.get_inner();
        let reader = inner.read().unwrap();
        for miniblock in initial_miniblock..current_miniblock {
            let actual_cached_value = reader
                .block_hashes
                .get(&miniblock)
                .map(|hash| {
                    reader
                        .previous_states
                        .get(hash)
                        .unwrap_or_else(|| panic!("state was not cached for block {}", miniblock))
                })
                .and_then(|state| state.get(&input_storage_key))
                .copied();

            assert_eq!(
                Some(input_storage_value),
                actual_cached_value,
                "unexpected cached state value for block {}",
                miniblock
            );
        }
    }

    #[tokio::test]
    async fn test_get_storage_fetches_zero_value_for_non_existent_key() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let value = node
            .get_storage(H160::repeat_byte(0xf1), U256::from(1024), None)
            .await
            .expect("failed retrieving storage");
        assert_eq!(H256::zero(), value);
    }

    #[tokio::test]
    async fn test_get_storage_uses_fork_to_get_value_for_historical_block() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_address = H160::repeat_byte(0x1);
        let input_storage_value = H256::repeat_byte(0xcd);
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getStorageAt",
                "params": [
                    format!("{:#x}", input_address),
                    "0x0",
                    { "blockNumber": "0x2" },
                ],
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "result": format!("{:#x}", input_storage_value),
            }),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        let actual_value = node
            .get_storage(
                input_address,
                U256::zero(),
                Some(zksync_types::api::BlockIdVariant::BlockNumberObject(
                    BlockNumberObject {
                        block_number: BlockNumber::Number(U64::from(2)),
                    },
                )),
            )
            .await
            .expect("failed retrieving storage");
        assert_eq!(input_storage_value, actual_value);
    }

    #[tokio::test]
    async fn test_get_storage_uses_archived_storage_to_get_value_for_missing_key() {
        let input_address = H160::repeat_byte(0x1);
        let input_storage_key = StorageKey::new(
            AccountTreeId::new(input_address),
            u256_to_h256(U256::zero()),
        );
        let input_storage_value = H256::repeat_byte(0xcd);

        let node = InMemoryNode::<HttpForkSource>::default();
        node.get_inner()
            .write()
            .map(|mut writer| {
                let historical_block = Block::<TransactionVariant> {
                    hash: H256::repeat_byte(0x2),
                    number: U64::from(2),
                    ..Default::default()
                };
                writer.block_hashes.insert(2, historical_block.hash);

                writer.previous_states.insert(
                    historical_block.hash,
                    hashmap! {
                        input_storage_key => input_storage_value,
                    },
                );
                writer
                    .blocks
                    .insert(historical_block.hash, historical_block);
            })
            .expect("failed setting storage for historical block");

        let actual_value = node
            .get_storage(
                input_address,
                U256::zero(),
                Some(zksync_types::api::BlockIdVariant::BlockNumberObject(
                    BlockNumberObject {
                        block_number: BlockNumber::Number(U64::from(2)),
                    },
                )),
            )
            .await
            .expect("failed retrieving storage");
        assert_eq!(input_storage_value, actual_value);
    }

    #[tokio::test]
    async fn test_get_storage_uses_fork_to_get_value_for_latest_block_for_missing_key() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_address = H160::repeat_byte(0x1);
        let input_storage_value = H256::repeat_byte(0xcd);
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getStorageAt",
                "params": [
                    format!("{:#x}", input_address),
                    "0x0",
                    "0xa",
                ],
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "result": format!("{:#x}", input_storage_value),
            }),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );
        node.get_inner()
            .write()
            .map(|mut writer| {
                let historical_block = Block::<TransactionVariant> {
                    hash: H256::repeat_byte(0x2),
                    number: U64::from(2),
                    ..Default::default()
                };
                writer.block_hashes.insert(2, historical_block.hash);
                writer
                    .previous_states
                    .insert(historical_block.hash, Default::default());
                writer
                    .blocks
                    .insert(historical_block.hash, historical_block);
            })
            .expect("failed setting storage for historical block");

        let actual_value = node
            .get_storage(
                input_address,
                U256::zero(),
                Some(zksync_types::api::BlockIdVariant::BlockNumberObject(
                    BlockNumberObject {
                        block_number: BlockNumber::Number(U64::from(2)),
                    },
                )),
            )
            .await
            .expect("failed retrieving storage");
        assert_eq!(input_storage_value, actual_value);
    }

    #[tokio::test]
    async fn test_get_storage_fetches_state_for_deployed_smart_contract_in_current_block() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let private_key = K256PrivateKey::from_bytes(H256::repeat_byte(0xef)).unwrap();
        let from_account = private_key.address();
        node.set_rich_account(from_account);

        let deployed_address = deployed_address_create(from_account, U256::zero());

        testing::deploy_contract(
            &node,
            H256::repeat_byte(0x1),
            &private_key,
            hex::decode(testing::STORAGE_CONTRACT_BYTECODE).unwrap(),
            None,
            Nonce(0),
        );

        let number1 = node
            .get_storage(deployed_address, U256::from(0), None)
            .await
            .expect("failed retrieving storage at slot 0");
        assert_eq!(U256::from(1024), h256_to_u256(number1));

        let number2 = node
            .get_storage(deployed_address, U256::from(1), None)
            .await
            .expect("failed retrieving storage at slot 1");
        assert_eq!(U256::MAX, h256_to_u256(number2));
    }

    #[tokio::test]
    async fn test_get_storage_fetches_state_for_deployed_smart_contract_in_old_block() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let private_key = K256PrivateKey::from_bytes(H256::repeat_byte(0xef)).unwrap();
        let from_account = private_key.address();
        node.set_rich_account(from_account);

        let deployed_address = deployed_address_create(from_account, U256::zero());

        let initial_block_hash = testing::deploy_contract(
            &node,
            H256::repeat_byte(0x1),
            &private_key,
            hex::decode(testing::STORAGE_CONTRACT_BYTECODE).unwrap(),
            None,
            Nonce(0),
        );

        // simulate a tx modifying the storage
        testing::apply_tx(&node, H256::repeat_byte(0x2));
        let key = StorageKey::new(
            AccountTreeId::new(deployed_address),
            u256_to_h256(U256::from(0)),
        );
        node.get_inner()
            .write()
            .unwrap()
            .fork_storage
            .inner
            .write()
            .unwrap()
            .raw_storage
            .state
            .insert(key, u256_to_h256(U256::from(512)));

        let number1_current = node
            .get_storage(deployed_address, U256::from(0), None)
            .await
            .expect("failed retrieving storage at slot 0");
        assert_eq!(U256::from(512), h256_to_u256(number1_current));

        let number1_old = node
            .get_storage(
                deployed_address,
                U256::from(0),
                Some(zksync_types::api::BlockIdVariant::BlockHashObject(
                    BlockHashObject {
                        block_hash: initial_block_hash,
                    },
                )),
            )
            .await
            .expect("failed retrieving storage at slot 0");
        assert_eq!(U256::from(1024), h256_to_u256(number1_old));
    }

    #[tokio::test]
    async fn test_get_filter_logs_returns_matching_logs_for_valid_id() {
        let node = InMemoryNode::<HttpForkSource>::default();

        // populate tx receipts with 2 tx each having logs
        {
            let inner = node.get_inner();
            let mut writer = inner.write().unwrap();
            writer.tx_results.insert(
                H256::repeat_byte(0x1),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: TransactionReceipt {
                        logs: vec![LogBuilder::new()
                            .set_address(H160::repeat_byte(0xa1))
                            .build()],
                        ..Default::default()
                    },
                    debug: default_tx_debug_info(),
                },
            );
            writer.tx_results.insert(
                H256::repeat_byte(0x2),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: TransactionReceipt {
                        logs: vec![
                            LogBuilder::new()
                                .set_address(H160::repeat_byte(0xa1))
                                .build(),
                            LogBuilder::new()
                                .set_address(H160::repeat_byte(0xa2))
                                .build(),
                        ],
                        ..Default::default()
                    },
                    debug: default_tx_debug_info(),
                },
            );
        }

        let filter_id = node
            .new_filter(Filter {
                address: Some(ValueOrArray(vec![H160::repeat_byte(0xa1)])),
                ..Default::default()
            })
            .await
            .expect("failed creating filter");

        match node
            .get_filter_logs(filter_id)
            .await
            .expect("failed getting filter changes")
        {
            FilterChanges::Logs(result) => assert_eq!(2, result.len()),
            changes => panic!("unexpected filter changes: {:?}", changes),
        }
    }

    #[tokio::test]
    async fn test_get_filter_logs_returns_error_for_invalid_id() {
        let node = InMemoryNode::<HttpForkSource>::default();

        // populate tx receipts with 2 tx each having logs
        {
            let inner = node.get_inner();
            let mut writer = inner.write().unwrap();
            writer.tx_results.insert(
                H256::repeat_byte(0x1),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: TransactionReceipt {
                        logs: vec![LogBuilder::new()
                            .set_address(H160::repeat_byte(0xa1))
                            .build()],
                        ..Default::default()
                    },
                    debug: default_tx_debug_info(),
                },
            );
        }

        let invalid_filter_id = U256::from(100);
        let result = node.get_filter_logs(invalid_filter_id).await;

        assert!(result.is_err(), "expected an error for invalid filter id");
    }

    #[tokio::test]
    async fn test_get_logs_returns_matching_logs() {
        let node = InMemoryNode::<HttpForkSource>::default();

        // populate tx receipts with 2 tx each having logs
        {
            let inner = node.get_inner();
            let mut writer = inner.write().unwrap();
            writer.tx_results.insert(
                H256::repeat_byte(0x1),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: TransactionReceipt {
                        logs: vec![LogBuilder::new()
                            .set_address(H160::repeat_byte(0xa1))
                            .build()],
                        ..Default::default()
                    },
                    debug: testing::default_tx_debug_info(),
                },
            );
            writer.tx_results.insert(
                H256::repeat_byte(0x2),
                TransactionResult {
                    info: testing::default_tx_execution_info(),
                    receipt: TransactionReceipt {
                        logs: vec![
                            LogBuilder::new()
                                .set_address(H160::repeat_byte(0xa1))
                                .build(),
                            LogBuilder::new()
                                .set_address(H160::repeat_byte(0xa2))
                                .build(),
                        ],
                        ..Default::default()
                    },
                    debug: testing::default_tx_debug_info(),
                },
            );
        }

        let result = node
            .get_logs(Filter {
                address: Some(ValueOrArray(vec![H160::repeat_byte(0xa2)])),
                ..Default::default()
            })
            .await
            .expect("failed getting filter changes");
        assert_eq!(1, result.len());

        let result = node
            .get_logs(Filter {
                address: Some(ValueOrArray(vec![H160::repeat_byte(0xa1)])),
                ..Default::default()
            })
            .await
            .expect("failed getting filter changes");
        assert_eq!(2, result.len());

        let result = node
            .get_logs(Filter {
                address: Some(ValueOrArray(vec![H160::repeat_byte(0x11)])),
                ..Default::default()
            })
            .await
            .expect("failed getting filter changes");
        assert_eq!(0, result.len());
    }

    #[tokio::test]
    async fn test_accounts() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let private_key = H256::repeat_byte(0x01);
        let from_account = K256PrivateKey::from_bytes(private_key).unwrap().address();
        node.set_rich_account(from_account);

        let account_result = node.accounts().await;
        let expected_accounts: Vec<H160> = vec![from_account];

        match account_result {
            Ok(accounts) => {
                assert_eq!(expected_accounts, accounts);
            }
            Err(e) => {
                panic!("Failed to fetch accounts: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_snapshot() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();
        let mut inner = inner.write().unwrap();

        inner
            .blocks
            .insert(H256::repeat_byte(0x1), Default::default());
        inner.block_hashes.insert(1, H256::repeat_byte(0x1));
        inner.tx_results.insert(
            H256::repeat_byte(0x1),
            TransactionResult {
                info: testing::default_tx_execution_info(),
                receipt: Default::default(),
                debug: testing::default_tx_debug_info(),
            },
        );
        inner.current_batch = 1;
        inner.current_miniblock = 1;
        inner.current_miniblock_hash = H256::repeat_byte(0x1);
        inner.current_timestamp = 1;
        inner
            .filters
            .add_block_filter()
            .expect("failed adding block filter");
        inner.impersonated_accounts.insert(H160::repeat_byte(0x1));
        inner.rich_accounts.insert(H160::repeat_byte(0x1));
        inner
            .previous_states
            .insert(H256::repeat_byte(0x1), Default::default());
        inner.fork_storage.set_value(
            StorageKey::new(AccountTreeId::new(H160::repeat_byte(0x1)), H256::zero()),
            H256::repeat_byte(0x1),
        );

        let storage = inner.fork_storage.inner.read().unwrap();
        let expected_snapshot = Snapshot {
            current_timestamp: inner.current_timestamp,
            current_batch: inner.current_batch,
            current_miniblock: inner.current_miniblock,
            current_miniblock_hash: inner.current_miniblock_hash,
            fee_input_provider: inner.fee_input_provider.clone(),
            tx_results: inner.tx_results.clone(),
            blocks: inner.blocks.clone(),
            block_hashes: inner.block_hashes.clone(),
            filters: inner.filters.clone(),
            impersonated_accounts: inner.impersonated_accounts.clone(),
            rich_accounts: inner.rich_accounts.clone(),
            previous_states: inner.previous_states.clone(),
            raw_storage: storage.raw_storage.clone(),
            value_read_cache: storage.value_read_cache.clone(),
            factory_dep_cache: storage.factory_dep_cache.clone(),
        };
        let actual_snapshot = inner.snapshot().expect("failed taking snapshot");

        assert_eq!(
            expected_snapshot.current_timestamp,
            actual_snapshot.current_timestamp
        );
        assert_eq!(
            expected_snapshot.current_batch,
            actual_snapshot.current_batch
        );
        assert_eq!(
            expected_snapshot.current_miniblock,
            actual_snapshot.current_miniblock
        );
        assert_eq!(
            expected_snapshot.current_miniblock_hash,
            actual_snapshot.current_miniblock_hash
        );
        assert_eq!(
            expected_snapshot.fee_input_provider,
            actual_snapshot.fee_input_provider
        );
        assert_eq!(
            expected_snapshot.tx_results.keys().collect_vec(),
            actual_snapshot.tx_results.keys().collect_vec()
        );
        assert_eq!(expected_snapshot.blocks, actual_snapshot.blocks);
        assert_eq!(expected_snapshot.block_hashes, actual_snapshot.block_hashes);
        assert_eq!(expected_snapshot.filters, actual_snapshot.filters);
        assert_eq!(
            expected_snapshot.impersonated_accounts,
            actual_snapshot.impersonated_accounts
        );
        assert_eq!(
            expected_snapshot.rich_accounts,
            actual_snapshot.rich_accounts
        );
        assert_eq!(
            expected_snapshot.previous_states,
            actual_snapshot.previous_states
        );
        assert_eq!(expected_snapshot.raw_storage, actual_snapshot.raw_storage);
        assert_eq!(
            expected_snapshot.value_read_cache,
            actual_snapshot.value_read_cache
        );
        assert_eq!(
            expected_snapshot.factory_dep_cache,
            actual_snapshot.factory_dep_cache
        );
    }

    #[tokio::test]
    async fn test_snapshot_restore() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let inner = node.get_inner();
        let mut inner = inner.write().unwrap();

        inner
            .blocks
            .insert(H256::repeat_byte(0x1), Default::default());
        inner.block_hashes.insert(1, H256::repeat_byte(0x1));
        inner.tx_results.insert(
            H256::repeat_byte(0x1),
            TransactionResult {
                info: testing::default_tx_execution_info(),
                receipt: Default::default(),
                debug: testing::default_tx_debug_info(),
            },
        );
        inner.current_batch = 1;
        inner.current_miniblock = 1;
        inner.current_miniblock_hash = H256::repeat_byte(0x1);
        inner.current_timestamp = 1;
        inner
            .filters
            .add_block_filter()
            .expect("failed adding block filter");
        inner.impersonated_accounts.insert(H160::repeat_byte(0x1));
        inner.rich_accounts.insert(H160::repeat_byte(0x1));
        inner
            .previous_states
            .insert(H256::repeat_byte(0x1), Default::default());
        inner.fork_storage.set_value(
            StorageKey::new(AccountTreeId::new(H160::repeat_byte(0x1)), H256::zero()),
            H256::repeat_byte(0x1),
        );

        let expected_snapshot = {
            let storage = inner.fork_storage.inner.read().unwrap();
            Snapshot {
                current_timestamp: inner.current_timestamp,
                current_batch: inner.current_batch,
                current_miniblock: inner.current_miniblock,
                current_miniblock_hash: inner.current_miniblock_hash,
                fee_input_provider: inner.fee_input_provider.clone(),
                tx_results: inner.tx_results.clone(),
                blocks: inner.blocks.clone(),
                block_hashes: inner.block_hashes.clone(),
                filters: inner.filters.clone(),
                impersonated_accounts: inner.impersonated_accounts.clone(),
                rich_accounts: inner.rich_accounts.clone(),
                previous_states: inner.previous_states.clone(),
                raw_storage: storage.raw_storage.clone(),
                value_read_cache: storage.value_read_cache.clone(),
                factory_dep_cache: storage.factory_dep_cache.clone(),
            }
        };

        // snapshot and modify node state
        let snapshot = inner.snapshot().expect("failed taking snapshot");
        inner
            .blocks
            .insert(H256::repeat_byte(0x2), Default::default());
        inner.block_hashes.insert(2, H256::repeat_byte(0x2));
        inner.tx_results.insert(
            H256::repeat_byte(0x2),
            TransactionResult {
                info: testing::default_tx_execution_info(),
                receipt: Default::default(),
                debug: default_tx_debug_info(),
            },
        );
        inner.current_batch = 2;
        inner.current_miniblock = 2;
        inner.current_miniblock_hash = H256::repeat_byte(0x2);
        inner.current_timestamp = 2;
        inner
            .filters
            .add_pending_transaction_filter()
            .expect("failed adding pending transaction filter");
        inner.impersonated_accounts.insert(H160::repeat_byte(0x2));
        inner.rich_accounts.insert(H160::repeat_byte(0x2));
        inner
            .previous_states
            .insert(H256::repeat_byte(0x2), Default::default());
        inner.fork_storage.set_value(
            StorageKey::new(AccountTreeId::new(H160::repeat_byte(0x2)), H256::zero()),
            H256::repeat_byte(0x2),
        );

        // restore
        inner
            .restore_snapshot(snapshot)
            .expect("failed restoring snapshot");

        let storage = inner.fork_storage.inner.read().unwrap();
        assert_eq!(expected_snapshot.current_timestamp, inner.current_timestamp);
        assert_eq!(expected_snapshot.current_batch, inner.current_batch);
        assert_eq!(expected_snapshot.current_miniblock, inner.current_miniblock);
        assert_eq!(
            expected_snapshot.current_miniblock_hash,
            inner.current_miniblock_hash
        );
        assert_eq!(
            expected_snapshot.fee_input_provider,
            inner.fee_input_provider
        );
        assert_eq!(
            expected_snapshot.tx_results.keys().collect_vec(),
            inner.tx_results.keys().collect_vec()
        );
        assert_eq!(expected_snapshot.blocks, inner.blocks);
        assert_eq!(expected_snapshot.block_hashes, inner.block_hashes);
        assert_eq!(expected_snapshot.filters, inner.filters);
        assert_eq!(
            expected_snapshot.impersonated_accounts,
            inner.impersonated_accounts
        );
        assert_eq!(expected_snapshot.rich_accounts, inner.rich_accounts);
        assert_eq!(expected_snapshot.previous_states, inner.previous_states);
        assert_eq!(expected_snapshot.raw_storage, storage.raw_storage);
        assert_eq!(expected_snapshot.value_read_cache, storage.value_read_cache);
        assert_eq!(
            expected_snapshot.factory_dep_cache,
            storage.factory_dep_cache
        );
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_hash_and_index_returns_none_for_invalid_block_hash() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let input_tx_hash = H256::repeat_byte(0x01);
        let (input_block_hash, _) = testing::apply_tx(&node, input_tx_hash);
        let invalid_block_hash = H256::repeat_byte(0xab);
        assert_ne!(input_block_hash, invalid_block_hash);

        let result = node
            .get_transaction_by_block_hash_and_index(invalid_block_hash, U64::from(0))
            .await
            .expect("failed fetching transaction");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_hash_and_index_returns_none_for_invalid_index() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let input_tx_hash = H256::repeat_byte(0x01);
        let (input_block_hash, _) = testing::apply_tx(&node, input_tx_hash);

        let result = node
            .get_transaction_by_block_hash_and_index(input_block_hash, U64::from(10))
            .await
            .expect("failed fetching transaction");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_hash_and_index_returns_transaction_for_valid_input() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let input_tx_hash = H256::repeat_byte(0x01);
        let (input_block_hash, _) = testing::apply_tx(&node, input_tx_hash);

        let actual_tx = node
            .get_transaction_by_block_hash_and_index(input_block_hash, U64::from(0))
            .await
            .expect("failed fetching transaction")
            .expect("no transaction");

        assert_eq!(input_tx_hash, actual_tx.hash);
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_hash_and_index_fetches_full_transaction_for_hash_from_fork(
    ) {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_block_hash = H256::repeat_byte(0x01);
        let input_tx_hash = H256::repeat_byte(0x02);
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getTransactionByHash",
                "params": [
                    format!("{:#x}", input_tx_hash),
                ],
            }),
            TransactionResponseBuilder::new()
                .set_hash(input_tx_hash)
                .set_block_hash(input_block_hash)
                .set_block_number(U64::from(1))
                .build(),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        // store the block info with just the tx hash invariant
        {
            let inner = node.get_inner();
            let mut writer = inner.write().unwrap();
            writer.blocks.insert(
                input_block_hash,
                Block {
                    transactions: vec![TransactionVariant::Hash(input_tx_hash)],
                    ..Default::default()
                },
            );
        }

        let actual_tx = node
            .get_transaction_by_block_hash_and_index(input_block_hash, U64::from(0))
            .await
            .expect("failed fetching transaction")
            .expect("no transaction");

        assert_eq!(input_tx_hash, actual_tx.hash);
        assert_eq!(Some(U64::from(1)), actual_tx.block_number);
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_hash_and_index_fetches_from_fork_if_block_missing() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_block_hash = H256::repeat_byte(0x01);
        let input_tx_hash = H256::repeat_byte(0x02);
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getTransactionByBlockHashAndIndex",
                "params": [
                    format!("{:#x}", input_block_hash),
                    "0x1"
                ],
            }),
            TransactionResponseBuilder::new()
                .set_hash(input_tx_hash)
                .set_block_hash(input_block_hash)
                .set_block_number(U64::from(100))
                .build(),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        let actual_tx = node
            .get_transaction_by_block_hash_and_index(input_block_hash, U64::from(1))
            .await
            .expect("failed fetching transaction")
            .expect("no transaction");

        assert_eq!(input_tx_hash, actual_tx.hash);
        assert_eq!(Some(U64::from(100)), actual_tx.block_number);
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_number_and_index_returns_none_for_invalid_block_number()
    {
        let node = InMemoryNode::<HttpForkSource>::default();
        let input_tx_hash = H256::repeat_byte(0x01);
        let (input_block_hash, _) = testing::apply_tx(&node, input_tx_hash);
        let invalid_block_hash = H256::repeat_byte(0xab);
        assert_ne!(input_block_hash, invalid_block_hash);

        let result = node
            .get_transaction_by_block_number_and_index(
                BlockNumber::Number(U64::from(100)),
                U64::from(0),
            )
            .await
            .expect("failed fetching transaction");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_number_and_index_returns_none_for_invalid_index() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let input_tx_hash = H256::repeat_byte(0x01);
        testing::apply_tx(&node, input_tx_hash);

        let result = node
            .get_transaction_by_block_number_and_index(BlockNumber::Latest, U64::from(10))
            .await
            .expect("failed fetching transaction");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_number_and_index_returns_transaction_for_valid_input() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let input_tx_hash = H256::repeat_byte(0x01);
        let (_, input_block_number) = testing::apply_tx(&node, input_tx_hash);

        let actual_tx = node
            .get_transaction_by_block_number_and_index(
                BlockNumber::Number(input_block_number),
                U64::from(0),
            )
            .await
            .expect("failed fetching transaction")
            .expect("no transaction");

        assert_eq!(input_tx_hash, actual_tx.hash);
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_number_and_index_fetches_full_transaction_for_hash_from_fork(
    ) {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_block_hash = H256::repeat_byte(0x01);
        let input_block_number = U64::from(100);
        let input_tx_hash = H256::repeat_byte(0x02);
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getTransactionByHash",
                "params": [
                    format!("{:#x}", input_tx_hash),
                ],
            }),
            TransactionResponseBuilder::new()
                .set_hash(input_tx_hash)
                .set_block_hash(input_block_hash)
                .set_block_number(input_block_number)
                .build(),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        // store the block info with just the tx hash invariant
        {
            let inner = node.get_inner();
            let mut writer = inner.write().unwrap();
            writer
                .block_hashes
                .insert(input_block_number.as_u64(), input_block_hash);
            writer.blocks.insert(
                input_block_hash,
                Block {
                    transactions: vec![TransactionVariant::Hash(input_tx_hash)],
                    ..Default::default()
                },
            );
        }

        let actual_tx = node
            .get_transaction_by_block_number_and_index(
                BlockNumber::Number(input_block_number),
                U64::from(0),
            )
            .await
            .expect("failed fetching transaction")
            .expect("no transaction");

        assert_eq!(input_tx_hash, actual_tx.hash);
        assert_eq!(Some(input_block_number), actual_tx.block_number);
    }

    #[tokio::test]
    async fn test_get_transaction_by_block_number_and_index_fetches_from_fork_if_block_missing() {
        let mock_server = MockServer::run_with_config(ForkBlockConfig {
            number: 10,
            transaction_count: 0,
            hash: H256::repeat_byte(0xab),
        });
        let input_block_hash = H256::repeat_byte(0x01);
        let input_block_number = U64::from(100);
        let input_tx_hash = H256::repeat_byte(0x02);
        mock_server.expect(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "eth_getTransactionByBlockNumberAndIndex",
                "params": [
                    format!("{:#x}", input_block_number),
                    "0x1"
                ],
            }),
            TransactionResponseBuilder::new()
                .set_hash(input_tx_hash)
                .set_block_hash(input_block_hash)
                .set_block_number(input_block_number)
                .build(),
        );

        let node = InMemoryNode::<HttpForkSource>::new(
            Some(ForkDetails::from_network(&mock_server.url(), None, CacheConfig::None).await),
            None,
            Default::default(),
            Default::default(),
        );

        let actual_tx = node
            .get_transaction_by_block_number_and_index(
                BlockNumber::Number(input_block_number),
                U64::from(1),
            )
            .await
            .expect("failed fetching transaction")
            .expect("no transaction");

        assert_eq!(input_tx_hash, actual_tx.hash);
        assert_eq!(Some(input_block_number), actual_tx.block_number);
    }

    #[tokio::test]
    async fn test_protocol_version_returns_currently_supported_version() {
        let node = InMemoryNode::<HttpForkSource>::default();

        let expected_version = String::from(PROTOCOL_VERSION);
        let actual_version = node
            .protocol_version()
            .await
            .expect("failed creating filter");

        assert_eq!(expected_version, actual_version);
    }
}
