use std::collections::{HashMap, HashSet};

use zksync_basic_types::{H160, H256, U256, U64};
use zksync_types::api::{BlockNumber, Log};
use zksync_web3_decl::types::FilterChanges;

#[derive(Debug, Clone)]
pub enum FilterType {
    Block(BlockFilter),
    Log(LogFilter),
    PendingTransaction(PendingTransactionFilter),
}

#[derive(Debug, Default, Clone)]
pub struct BlockFilter {
    updates: Vec<H256>,
}

#[derive(Debug, Clone)]
pub struct LogFilter {
    from_block: BlockNumber,
    to_block: BlockNumber,
    addresses: Vec<H160>,
    topics: [Option<HashSet<H256>>; 4],
    updates: Vec<Log>,
}

impl LogFilter {
    fn matches(&self, log: &Log, latest_block_number: U64) -> bool {
        let from = match self.from_block {
            BlockNumber::Finalized
            | BlockNumber::Pending
            | BlockNumber::Committed
            | BlockNumber::Latest => latest_block_number,
            BlockNumber::Earliest => U64::zero(),
            BlockNumber::Number(n) => n,
        };
        let to = match self.to_block {
            BlockNumber::Finalized
            | BlockNumber::Pending
            | BlockNumber::Committed
            | BlockNumber::Latest => latest_block_number,
            BlockNumber::Earliest => U64::zero(),
            BlockNumber::Number(n) => n,
        };

        let n = log.block_number.expect("block number must exist");
        if n < from || n > to {
            return false;
        }

        if !self.addresses.is_empty()
            && self.addresses.iter().all(|address| address != &log.address)
        {
            return false;
        }

        let mut matched_topic = [true; 4];
        for (i, topic) in log.topics.iter().take(4).enumerate() {
            if let Some(topic_set) = &self.topics[i] {
                if !topic_set.contains(topic) {
                    matched_topic[i] = false;
                }
            }
        }

        matched_topic.iter().all(|m| *m == true)
    }
}

#[derive(Debug, Default, Clone)]
pub struct PendingTransactionFilter {
    updates: Vec<H256>,
}

type Result<T> = std::result::Result<T, &'static str>;

#[derive(Debug, Default, Clone)]
pub struct EthFilters {
    id_counter: U256,
    filters: HashMap<U256, FilterType>,
}

impl EthFilters {
    /// Adds a block filter to keep track of new block hashes. Returns the filter id.
    pub fn add_block_filter(&mut self) -> Result<U256> {
        self.id_counter = self
            .id_counter
            .checked_add(U256::from(1))
            .ok_or("overflow")?;
        self.filters.insert(
            self.id_counter,
            FilterType::Block(BlockFilter {
                updates: Default::default(),
            }),
        );

        log::info!("created block filter '{:#x}'", self.id_counter);
        Ok(self.id_counter)
    }

    /// Adds a log filter to keep track of new transaction logs. Returns the filter id.
    pub fn add_log_filter(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
        addresses: Vec<H160>,
        topics: [Option<HashSet<H256>>; 4],
    ) -> Result<U256> {
        self.id_counter = self
            .id_counter
            .checked_add(U256::from(1))
            .ok_or("overflow")?;
        self.filters.insert(
            self.id_counter,
            FilterType::Log(LogFilter {
                from_block,
                to_block,
                addresses,
                topics,
                updates: Default::default(),
            }),
        );

        log::info!("created log filter '{:#x}'", self.id_counter);
        Ok(self.id_counter)
    }

    /// Adds a filter to keep track of new pending transaction hashes. Returns the filter id.
    pub fn add_pending_transaction_filter(&mut self) -> Result<U256> {
        self.id_counter = self
            .id_counter
            .checked_add(U256::from(1))
            .ok_or("overflow")?;
        self.filters.insert(
            self.id_counter,
            FilterType::PendingTransaction(PendingTransactionFilter {
                updates: Default::default(),
            }),
        );

        log::info!(
            "created pending transaction filter '{:#x}'",
            self.id_counter
        );
        Ok(self.id_counter)
    }

    /// Removes the filter with the given id. Returns true if the filter existed, false otherwise.
    pub fn remove_filter(&mut self, id: U256) -> bool {
        log::info!("removing filter '{id:#x}'");
        self.filters.remove(&id).is_some()
    }

    /// Retrieves the filter updates with the given id. The updates are reset after this call.
    pub fn get_new_changes(&mut self, id: U256) -> Result<FilterChanges> {
        let filter = self.filters.get_mut(&id).ok_or("invalid filter")?;
        let changes = match filter {
            FilterType::Block(f) => {
                if f.updates.is_empty() {
                    FilterChanges::Empty(Default::default())
                } else {
                    let updates = f.updates.clone();
                    f.updates.clear();
                    FilterChanges::Hashes(updates)
                }
            }
            FilterType::Log(f) => {
                if f.updates.is_empty() {
                    FilterChanges::Empty(Default::default())
                } else {
                    let updates = f.updates.clone();
                    f.updates.clear();
                    FilterChanges::Logs(updates)
                }
            }
            FilterType::PendingTransaction(f) => {
                if f.updates.is_empty() {
                    FilterChanges::Empty(Default::default())
                } else {
                    let updates = f.updates.clone();
                    f.updates.clear();
                    FilterChanges::Hashes(updates)
                }
            }
        };

        Ok(changes)
    }

    pub fn notify_new_block(&mut self, hash: H256) {
        self.filters
            .iter_mut()
            .for_each(|(_, filter)| match filter {
                FilterType::Block(f) => f.updates.push(hash),
                _ => (),
            })
    }

    pub fn notify_new_pending_transaction(&mut self, hash: H256) {
        self.filters
            .iter_mut()
            .for_each(|(_, filter)| match filter {
                FilterType::PendingTransaction(f) => f.updates.push(hash),
                _ => (),
            })
    }

    pub fn notify_new_log(&mut self, log: &Log, latest_block_number: U64) {
        self.filters
            .iter_mut()
            .for_each(|(_, filter)| match filter {
                FilterType::Log(f) => {
                    if f.matches(&log, latest_block_number) {
                        f.updates.push(log.clone());
                    }
                }
                _ => (),
            })
    }
}
