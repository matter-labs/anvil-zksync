use std::collections::{HashMap, HashSet};

use zksync_basic_types::{H160, H256, U256, U64};
use zksync_types::api::{BlockNumber, Log};
use zksync_web3_decl::types::FilterChanges;

use crate::utils;

/// Specifies a filter type
#[derive(Debug, Clone, PartialEq)]
pub enum FilterType {
    /// A filter for block information
    Block(BlockFilter),
    /// A filter for log information.
    /// This is [Box] to ensure the enum invariants are similar size.
    Log(Box<LogFilter>),
    /// A filter for pending transaction information
    PendingTransaction(PendingTransactionFilter),
}

/// Specifies a filter that keeps track of new blocks
#[derive(Debug, Default, Clone, PartialEq)]
pub struct BlockFilter {
    updates: Vec<H256>,
}

/// Specifies a filter that keeps track of new logs
#[derive(Debug, Clone, PartialEq)]
pub struct LogFilter {
    from_block: BlockNumber,
    to_block: BlockNumber,
    addresses: Vec<H160>,
    topics: [Option<HashSet<H256>>; 4],
    updates: Vec<Log>,
}

impl LogFilter {
    pub fn new(
        from_block: BlockNumber,
        to_block: BlockNumber,
        addresses: Vec<H160>,
        topics: [Option<HashSet<H256>>; 4],
    ) -> Self {
        Self {
            from_block,
            to_block,
            addresses,
            topics,
            updates: Default::default(),
        }
    }

    pub fn matches(&self, log: &Log, latest_block_number: U64) -> bool {
        let from = utils::to_real_block_number(self.from_block, latest_block_number);
        let to = utils::to_real_block_number(self.to_block, latest_block_number);

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
                if !topic_set.is_empty() && !topic_set.contains(topic) {
                    matched_topic[i] = false;
                }
            }
        }

        matched_topic.iter().all(|m| *m)
    }
}

/// Specifies a filter that keeps track of new pending transactions
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PendingTransactionFilter {
    updates: Vec<H256>,
}

type Result<T> = std::result::Result<T, &'static str>;

/// Keeps track of installed filters and their respective updates.
#[derive(Debug, Default, Clone, PartialEq)]
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

        tracing::info!("created block filter '{:#x}'", self.id_counter);
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
            FilterType::Log(Box::new(LogFilter {
                from_block,
                to_block,
                addresses,
                topics,
                updates: Default::default(),
            })),
        );

        tracing::info!("created log filter '{:#x}'", self.id_counter);
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

        tracing::info!(
            "created pending transaction filter '{:#x}'",
            self.id_counter
        );
        Ok(self.id_counter)
    }

    /// Removes the filter with the given id. Returns true if the filter existed, false otherwise.
    pub fn remove_filter(&mut self, id: U256) -> bool {
        tracing::info!("removing filter '{id:#x}'");
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

    pub fn get_filter(&self, id: U256) -> Option<&FilterType> {
        self.filters.get(&id)
    }

    /// Notify available filters of a newly produced block
    pub fn notify_new_block(&mut self, hash: H256) {
        self.filters.iter_mut().for_each(|(_, filter)| {
            if let FilterType::Block(f) = filter {
                f.updates.push(hash)
            }
        })
    }

    /// Notify available filters of a new pending transaction
    pub fn notify_new_pending_transaction(&mut self, hash: H256) {
        self.filters.iter_mut().for_each(|(_, filter)| {
            if let FilterType::PendingTransaction(f) = filter {
                f.updates.push(hash)
            }
        })
    }

    /// Notify available filters of a new transaction log
    pub fn notify_new_log(&mut self, log: &Log, latest_block_number: U64) {
        self.filters.iter_mut().for_each(|(_, filter)| {
            if let FilterType::Log(f) = filter {
                if f.matches(log, latest_block_number) {
                    f.updates.push(log.clone());
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::LogBuilder;

    use super::*;

    use maplit::{hashmap, hashset};

    #[test]
    fn test_add_block_filter() {
        let mut filters = EthFilters::default();
        let id = filters.add_block_filter().expect("failed adding filter");

        assert_eq!(U256::from(1), id);
        assert_eq!(
            hashmap! {
                U256::from(1) => FilterType::Block(BlockFilter { updates: vec![] })
            },
            filters.filters
        );
    }

    #[test]
    fn test_add_log_filter() {
        let mut filters = EthFilters::default();
        let id = filters
            .add_log_filter(
                BlockNumber::Latest,
                BlockNumber::Number(U64::from(10)),
                vec![H160::repeat_byte(0x1)],
                [
                    Some(hashset! { H256::repeat_byte(0x2) }),
                    Some(hashset! { H256::repeat_byte(0x3), H256::repeat_byte(0x4) }),
                    None,
                    Some(hashset! {}),
                ],
            )
            .expect("failed adding filter");

        assert_eq!(U256::from(1), id);
        assert_eq!(
            hashmap! {
                U256::from(1) => FilterType::Log(Box::new(LogFilter {
                    from_block: BlockNumber::Latest,
                    to_block: BlockNumber::Number(U64::from(10)),
                    addresses: vec![H160::repeat_byte(0x1)],
                    topics: [
                        Some(hashset! { H256::repeat_byte(0x2) }),
                        Some(hashset! { H256::repeat_byte(0x3), H256::repeat_byte(0x4) }),
                        None,
                        Some(hashset! {}),
                    ],
                    updates:vec![],
                }))
            },
            filters.filters
        );
    }

    #[test]
    fn test_add_pending_transaction_filter() {
        let mut filters = EthFilters::default();
        let id = filters
            .add_pending_transaction_filter()
            .expect("failed adding filter");

        assert_eq!(U256::from(1), id);
        assert_eq!(
            hashmap! {
                U256::from(1) => FilterType::PendingTransaction(PendingTransactionFilter { updates: vec![] })
            },
            filters.filters
        );
    }

    #[test]
    fn test_different_filters_share_incremental_identifiers() {
        let mut filters = EthFilters::default();

        let block_filter_id = filters.add_block_filter().expect("failed adding filter");
        let log_filter_id = filters
            .add_log_filter(
                BlockNumber::Earliest,
                BlockNumber::Latest,
                Default::default(),
                Default::default(),
            )
            .expect("failed adding filter");
        let pending_transaction_filter_id = filters
            .add_pending_transaction_filter()
            .expect("failed adding filter");

        assert_eq!(U256::from(1), block_filter_id);
        assert_eq!(U256::from(2), log_filter_id);
        assert_eq!(U256::from(3), pending_transaction_filter_id);
    }

    #[test]
    fn test_remove_filter() {
        let mut filters = EthFilters::default();
        let block_filter_id = filters.add_block_filter().expect("failed adding filter");
        let log_filter_id = filters
            .add_log_filter(
                BlockNumber::Earliest,
                BlockNumber::Latest,
                Default::default(),
                Default::default(),
            )
            .expect("failed adding filter");
        let pending_transaction_filter_id = filters
            .add_pending_transaction_filter()
            .expect("failed adding filter");

        filters.remove_filter(log_filter_id);

        assert!(
            filters.filters.contains_key(&block_filter_id),
            "filter was erroneously removed"
        );
        assert!(
            filters.filters.contains_key(&pending_transaction_filter_id),
            "filter was erroneously removed"
        );
        assert!(
            !filters.filters.contains_key(&log_filter_id),
            "filter was not removed"
        );
    }

    #[test]
    fn test_notify_new_block_appends_updates() {
        let mut filters = EthFilters::default();
        let id = filters.add_block_filter().expect("failed adding filter");

        filters.notify_new_block(H256::repeat_byte(0x1));

        match filters.filters.get(&id).unwrap() {
            FilterType::Block(f) => {
                assert_eq!(vec![H256::repeat_byte(0x1)], f.updates);
            }
            _ => panic!("invalid filter"),
        }
    }

    #[test]
    fn test_notify_new_log_appends_matching_updates() {
        let mut filters = EthFilters::default();
        let match_address = H160::repeat_byte(0x1);
        let id = filters
            .add_log_filter(
                BlockNumber::Earliest,
                BlockNumber::Latest,
                vec![match_address],
                Default::default(),
            )
            .expect("failed adding filter");

        let log = LogBuilder::new()
            .set_address(match_address)
            .set_block(U64::from(1))
            .build();
        filters.notify_new_log(&log, U64::from(1));

        match filters.filters.get(&id).unwrap() {
            FilterType::Log(f) => {
                assert_eq!(vec![log], f.updates);
            }
            _ => panic!("invalid filter"),
        }
    }

    #[test]
    fn test_notify_new_pending_transaction_appends_updates() {
        let mut filters = EthFilters::default();
        let id = filters
            .add_pending_transaction_filter()
            .expect("failed adding filter");

        filters.notify_new_pending_transaction(H256::repeat_byte(0x1));

        match filters.filters.get(&id).unwrap() {
            FilterType::PendingTransaction(f) => {
                assert_eq!(vec![H256::repeat_byte(0x1)], f.updates);
            }
            _ => panic!("invalid filter"),
        }
    }

    #[test]
    fn test_get_new_changes_block_returns_updates_and_clears_them() {
        let mut filters = EthFilters::default();
        let id = filters.add_block_filter().expect("failed adding filter");
        filters.notify_new_block(H256::repeat_byte(0x1));

        let changes = filters
            .get_new_changes(id)
            .expect("failed retrieving changes");

        match changes {
            FilterChanges::Hashes(result) => {
                assert_eq!(vec![H256::repeat_byte(0x1)], result);
            }
            _ => panic!("unexpected filter changes {:?}", changes),
        }
        match filters.filters.get(&id).unwrap() {
            FilterType::Block(f) => {
                assert!(f.updates.is_empty(), "updates were not cleared");
            }
            _ => panic!("invalid filter"),
        }
    }

    #[test]
    fn test_get_new_changes_log_appends_mreturng_updates_and_clears_them() {
        let mut filters = EthFilters::default();
        let match_address = H160::repeat_byte(0x1);
        let id = filters
            .add_log_filter(
                BlockNumber::Earliest,
                BlockNumber::Latest,
                vec![match_address],
                Default::default(),
            )
            .expect("failed adding filter");

        let log = LogBuilder::new()
            .set_address(match_address)
            .set_block(U64::from(1))
            .build();
        filters.notify_new_log(&log, U64::from(1));

        let changes = filters
            .get_new_changes(id)
            .expect("failed retrieving changes");

        match changes {
            FilterChanges::Logs(result) => {
                assert_eq!(vec![log], result);
            }
            _ => panic!("unexpected filter changes {:?}", changes),
        }
        match filters.filters.get(&id).unwrap() {
            FilterType::Log(f) => {
                assert!(f.updates.is_empty(), "updates were not cleared");
            }
            _ => panic!("invalid filter"),
        }
    }

    #[test]
    fn test_get_new_changes_pending_transaction_returns_updates_and_clears_them() {
        let mut filters = EthFilters::default();
        let id = filters
            .add_pending_transaction_filter()
            .expect("failed adding filter");

        filters.notify_new_pending_transaction(H256::repeat_byte(0x1));

        let changes = filters
            .get_new_changes(id)
            .expect("failed retrieving changes");

        match changes {
            FilterChanges::Hashes(result) => {
                assert_eq!(vec![H256::repeat_byte(0x1)], result);
            }
            _ => panic!("unexpected filter changes {:?}", changes),
        }
        match filters.filters.get(&id).unwrap() {
            FilterType::PendingTransaction(f) => {
                assert!(f.updates.is_empty(), "updates were not cleared");
            }
            _ => panic!("invalid filter"),
        }
    }
}

#[cfg(test)]
mod log_filter_tests {
    use maplit::hashset;

    use crate::testing::LogBuilder;

    use super::*;

    #[test]
    fn test_filter_from_block_earliest_accepts_all_block_numbers_lte_latest() {
        let filter = LogFilter {
            from_block: BlockNumber::Earliest,
            to_block: BlockNumber::Latest,
            addresses: Default::default(),
            topics: Default::default(),
            updates: Default::default(),
        };

        let latest_block_number = 2u64;
        for log_block in 0..=latest_block_number {
            let matched = filter.matches(
                &LogBuilder::new().set_block(U64::from(log_block)).build(),
                U64::from(latest_block_number),
            );
            assert!(
                matched,
                "failed matching log for block_number {}",
                log_block
            );
        }
    }

    #[test]
    fn test_filter_from_block_latest_accepts_block_number_eq_latest() {
        let filter = LogFilter {
            from_block: BlockNumber::Latest,
            to_block: BlockNumber::Latest,
            addresses: Default::default(),
            topics: Default::default(),
            updates: Default::default(),
        };

        let latest_block_number = U64::from(2);
        let input_block_number = U64::from(2);
        let matched = filter.matches(
            &LogBuilder::new().set_block(input_block_number).build(),
            latest_block_number,
        );
        assert!(matched);
    }

    #[test]
    fn test_filter_from_block_latest_rejects_block_number_lt_latest() {
        let filter = LogFilter {
            from_block: BlockNumber::Latest,
            to_block: BlockNumber::Latest,
            addresses: Default::default(),
            topics: Default::default(),
            updates: Default::default(),
        };

        let latest_block_number = U64::from(2);
        let matched = filter.matches(
            &LogBuilder::new().set_block(U64::from(1)).build(),
            latest_block_number,
        );
        assert!(!matched);
    }

    #[test]
    fn test_filter_from_block_accepts_all_block_numbers_gte_input_number() {
        let input_block_number = 2u64;
        let latest_block_number = 100u64;
        let filter = LogFilter {
            from_block: BlockNumber::Number(U64::from(input_block_number)),
            to_block: BlockNumber::Latest,
            addresses: Default::default(),
            topics: Default::default(),
            updates: Default::default(),
        };

        for log_block in input_block_number..=input_block_number + 1 {
            let matched = filter.matches(
                &LogBuilder::new().set_block(U64::from(log_block)).build(),
                U64::from(latest_block_number),
            );
            assert!(
                matched,
                "failed matching log for block_number {}",
                log_block
            );
        }
    }

    #[test]
    fn test_filter_from_block_rejects_block_number_lt_input_number() {
        let input_block_number = 2u64;
        let latest_block_number = 100u64;
        let filter = LogFilter {
            from_block: BlockNumber::Number(U64::from(input_block_number)),
            to_block: BlockNumber::Latest,
            addresses: Default::default(),
            topics: Default::default(),
            updates: Default::default(),
        };

        let matched = filter.matches(
            &LogBuilder::new()
                .set_block(U64::from(input_block_number - 1))
                .build(),
            U64::from(latest_block_number),
        );
        assert!(!matched);
    }

    #[test]
    fn test_filter_to_block_latest_accepts_block_number_lte_latest() {
        let filter = LogFilter {
            from_block: BlockNumber::Earliest,
            to_block: BlockNumber::Latest,
            addresses: Default::default(),
            topics: Default::default(),
            updates: Default::default(),
        };

        let latest_block_number = 2u32;
        for log_block in 1..=latest_block_number {
            let matched = filter.matches(
                &LogBuilder::new().set_block(U64::from(log_block)).build(),
                U64::from(latest_block_number),
            );
            assert!(
                matched,
                "failed matching log for block_number {}",
                log_block
            );
        }
    }

    #[test]
    fn test_filter_to_block_accepts_all_block_numbers_lte_input_number() {
        let input_block_number = 2u64;
        let latest_block_number = 100u64;
        let filter = LogFilter {
            from_block: BlockNumber::Earliest,
            to_block: BlockNumber::Number(U64::from(input_block_number)),
            addresses: Default::default(),
            topics: Default::default(),
            updates: Default::default(),
        };

        for log_block in input_block_number - 1..=input_block_number {
            let matched = filter.matches(
                &LogBuilder::new().set_block(U64::from(log_block)).build(),
                U64::from(latest_block_number),
            );
            assert!(
                matched,
                "failed matching log for block_number {}",
                log_block
            );
        }
    }

    #[test]
    fn test_filter_to_block_rejects_block_number_gt_input_number() {
        let input_block_number = 2u64;
        let latest_block_number = 100u64;
        let filter = LogFilter {
            from_block: BlockNumber::Earliest,
            to_block: BlockNumber::Number(U64::from(input_block_number)),
            addresses: Default::default(),
            topics: Default::default(),
            updates: Default::default(),
        };

        let matched = filter.matches(
            &LogBuilder::new()
                .set_block(U64::from(input_block_number + 1))
                .build(),
            U64::from(latest_block_number),
        );
        assert!(!matched);
    }

    #[test]
    fn test_filter_from_and_to_block_rejects_block_number_left_of_range() {
        let input_from_block_number = 2u64;
        let input_to_block_number = 4u64;
        let latest_block_number = 100u64;
        let filter = LogFilter {
            from_block: BlockNumber::Number(U64::from(input_from_block_number)),
            to_block: BlockNumber::Number(U64::from(input_to_block_number)),
            addresses: Default::default(),
            topics: Default::default(),
            updates: Default::default(),
        };

        let matched = filter.matches(
            &LogBuilder::new()
                .set_block(U64::from(input_from_block_number - 1))
                .build(),
            U64::from(latest_block_number),
        );
        assert!(!matched);
    }

    #[test]
    fn test_filter_from_and_to_block_rejects_block_number_right_of_range() {
        let input_from_block_number = 2u64;
        let input_to_block_number = 4u64;
        let latest_block_number = 100u64;
        let filter = LogFilter {
            from_block: BlockNumber::Number(U64::from(input_from_block_number)),
            to_block: BlockNumber::Number(U64::from(input_to_block_number)),
            addresses: Default::default(),
            topics: Default::default(),
            updates: Default::default(),
        };

        let matched = filter.matches(
            &LogBuilder::new()
                .set_block(U64::from(input_to_block_number + 1))
                .build(),
            U64::from(latest_block_number),
        );
        assert!(!matched);
    }

    #[test]
    fn test_filter_from_and_to_block_accepts_block_number_inclusive_of_range() {
        let input_from_block_number = 2u64;
        let input_to_block_number = 4u64;
        let latest_block_number = 100u64;
        let filter = LogFilter {
            from_block: BlockNumber::Number(U64::from(input_from_block_number)),
            to_block: BlockNumber::Number(U64::from(input_to_block_number)),
            addresses: Default::default(),
            topics: Default::default(),
            updates: Default::default(),
        };

        for log_block in input_from_block_number..=input_to_block_number {
            let matched = filter.matches(
                &LogBuilder::new().set_block(U64::from(log_block)).build(),
                U64::from(latest_block_number),
            );
            assert!(
                matched,
                "failed matching log for block_number {}",
                log_block
            );
        }
    }

    #[test]
    fn test_filter_address_rejects_if_address_not_in_set() {
        let filter = LogFilter {
            from_block: BlockNumber::Earliest,
            to_block: BlockNumber::Latest,
            addresses: vec![H160::repeat_byte(0xa)],
            topics: Default::default(),
            updates: Default::default(),
        };

        let matched = filter.matches(
            &LogBuilder::new()
                .set_address(H160::repeat_byte(0x1))
                .build(),
            U64::from(100),
        );
        assert!(!matched);
    }

    #[test]
    fn test_filter_address_accepts_if_address_in_set() {
        let filter = LogFilter {
            from_block: BlockNumber::Earliest,
            to_block: BlockNumber::Latest,
            addresses: vec![H160::repeat_byte(0x1)],
            topics: Default::default(),
            updates: Default::default(),
        };

        let matched = filter.matches(
            &LogBuilder::new()
                .set_address(H160::repeat_byte(0x1))
                .build(),
            U64::from(100),
        );
        assert!(matched);
    }

    #[test]
    fn test_filter_address_accepts_if_address_set_empty() {
        let filter = LogFilter {
            from_block: BlockNumber::Earliest,
            to_block: BlockNumber::Latest,
            addresses: vec![],
            topics: Default::default(),
            updates: Default::default(),
        };

        let matched = filter.matches(
            &LogBuilder::new()
                .set_address(H160::repeat_byte(0x1))
                .build(),
            U64::from(100),
        );
        assert!(matched);
    }

    #[test]
    fn test_filter_topic_none_accepts_any_topic() {
        let filter = LogFilter {
            from_block: BlockNumber::Earliest,
            to_block: BlockNumber::Latest,
            addresses: Default::default(),
            topics: [None, None, None, None],
            updates: Default::default(),
        };

        for topic_idx in 0..4 {
            let mut input_topics = [H256::zero(); 4].to_vec();
            input_topics[topic_idx] = H256::repeat_byte(0x1);

            let matched = filter.matches(
                &LogBuilder::new().set_topics(input_topics).build(),
                U64::from(100),
            );
            assert!(matched, "failed matching log for topic index {}", topic_idx);
        }
    }

    #[test]
    fn test_filter_topic_exactly_one_accepts_exact_topic() {
        for topic_idx in 0..4 {
            let match_topic = H256::repeat_byte(0x1);
            let mut topics = [None, None, None, None];
            topics[topic_idx] = Some(hashset! { match_topic });
            let filter = LogFilter {
                from_block: BlockNumber::Earliest,
                to_block: BlockNumber::Latest,
                addresses: Default::default(),
                topics,
                updates: Default::default(),
            };
            let mut input_topics = [H256::zero(); 4].to_vec();
            input_topics[topic_idx] = match_topic;

            let matched = filter.matches(
                &LogBuilder::new().set_topics(input_topics).build(),
                U64::from(100),
            );
            assert!(matched, "failed matching log for topic index {}", topic_idx);
        }
    }

    #[test]
    fn test_filter_topic_multiple_accepts_either_topic() {
        for topic_idx in 0..4 {
            let match_topic_1 = H256::repeat_byte(0x1);
            let match_topic_2 = H256::repeat_byte(0x2);
            let mut topics = [None, None, None, None];
            topics[topic_idx] = Some(hashset! { match_topic_1, match_topic_2, });
            let filter = LogFilter {
                from_block: BlockNumber::Earliest,
                to_block: BlockNumber::Latest,
                addresses: Default::default(),
                topics,
                updates: Default::default(),
            };

            let mut input_topics = [H256::zero(); 4].to_vec();
            input_topics[topic_idx] = match_topic_1;
            let matched = filter.matches(
                &LogBuilder::new().set_topics(input_topics).build(),
                U64::from(100),
            );
            assert!(
                matched,
                "failed matching log for topic index {} for {}",
                topic_idx, match_topic_1
            );

            let mut input_topics = [H256::zero(); 4].to_vec();
            input_topics[topic_idx] = match_topic_2;
            let matched = filter.matches(
                &LogBuilder::new().set_topics(input_topics).build(),
                U64::from(100),
            );
            assert!(
                matched,
                "failed matching log for topic index {} for {}",
                topic_idx, match_topic_2
            );
        }
    }

    #[test]
    fn test_filter_topic_exactly_one_rejects_different_topic() {
        for topic_idx in 0..4 {
            let match_topic = H256::repeat_byte(0x1);
            let mut topics = [None, None, None, None];
            topics[topic_idx] = Some(hashset! { match_topic });
            let filter = LogFilter {
                from_block: BlockNumber::Earliest,
                to_block: BlockNumber::Latest,
                addresses: Default::default(),
                topics,
                updates: Default::default(),
            };
            let mut input_topics = [H256::zero(); 4].to_vec();
            input_topics[topic_idx] = H256::repeat_byte(0xa);

            let matched = filter.matches(
                &LogBuilder::new().set_topics(input_topics).build(),
                U64::from(100),
            );
            assert!(
                !matched,
                "erroneously matched log for topic index {}",
                topic_idx
            );
        }
    }

    #[test]
    fn test_filter_topic_multiple_rejects_different_topic() {
        for topic_idx in 0..4 {
            let match_topic_1 = H256::repeat_byte(0x1);
            let match_topic_2 = H256::repeat_byte(0x2);
            let mut topics = [None, None, None, None];
            topics[topic_idx] = Some(hashset! { match_topic_1, match_topic_2, });
            let filter = LogFilter {
                from_block: BlockNumber::Earliest,
                to_block: BlockNumber::Latest,
                addresses: Default::default(),
                topics,
                updates: Default::default(),
            };

            let mut input_topics = [H256::zero(); 4].to_vec();
            input_topics[topic_idx] = H256::repeat_byte(0xa);
            let matched = filter.matches(
                &LogBuilder::new().set_topics(input_topics).build(),
                U64::from(100),
            );
            assert!(
                !matched,
                "erroneously matched log for topic index {}",
                topic_idx
            );
        }
    }

    #[test]
    fn test_filter_topic_combination_accepts_if_all_topics_valid() {
        let filter = LogFilter {
            from_block: BlockNumber::Earliest,
            to_block: BlockNumber::Latest,
            addresses: Default::default(),
            topics: [
                None, // matches anything
                Some(hashset! { H256::repeat_byte(0x1) }),
                Some(hashset! { H256::repeat_byte(0x2), H256::repeat_byte(0x3) }),
                Some(hashset! {}), // matches anything
            ],
            updates: Default::default(),
        };

        let matched = filter.matches(
            &LogBuilder::new()
                .set_topics(vec![
                    H256::zero(),
                    H256::repeat_byte(0x1),
                    H256::repeat_byte(0x3),
                    H256::repeat_byte(0xa),
                ])
                .build(),
            U64::from(100),
        );
        assert!(matched);
    }

    #[test]
    fn test_filter_topic_combination_rejects_if_any_topic_is_invalid() {
        let filter = LogFilter {
            from_block: BlockNumber::Earliest,
            to_block: BlockNumber::Latest,
            addresses: Default::default(),
            topics: [
                None, // matches anything
                Some(hashset! { H256::repeat_byte(0x1) }),
                Some(hashset! { H256::repeat_byte(0x2), H256::repeat_byte(0x3) }),
                Some(hashset! {}), // matches anything
            ],
            updates: Default::default(),
        };

        let matched = filter.matches(
            &LogBuilder::new()
                .set_topics(vec![
                    H256::zero(),
                    H256::repeat_byte(0xf), // invalid
                    H256::repeat_byte(0x3),
                    H256::repeat_byte(0xa),
                ])
                .build(),
            U64::from(100),
        );
        assert!(!matched, "erroneously matched on invalid topic1");

        let matched = filter.matches(
            &LogBuilder::new()
                .set_topics(vec![
                    H256::zero(),
                    H256::repeat_byte(0x1),
                    H256::repeat_byte(0xf), // invalid
                    H256::repeat_byte(0xa),
                ])
                .build(),
            U64::from(100),
        );
        assert!(!matched, "erroneously matched on invalid topic2");
    }

    #[test]
    fn test_filter_combination_accepts_if_all_criteria_valid() {
        let filter = LogFilter {
            from_block: BlockNumber::Number(U64::from(2)),
            to_block: BlockNumber::Number(U64::from(5)),
            addresses: vec![H160::repeat_byte(0xab)],
            topics: [
                None,
                Some(hashset! { H256::repeat_byte(0x1) }),
                Some(hashset! { H256::repeat_byte(0x2), H256::repeat_byte(0x3) }),
                Some(hashset! {}),
            ],
            updates: Default::default(),
        };

        let matched = filter.matches(
            &LogBuilder::new()
                .set_block(U64::from(4))
                .set_address(H160::repeat_byte(0xab))
                .set_topics(vec![
                    H256::zero(),
                    H256::repeat_byte(0x1),
                    H256::repeat_byte(0x3),
                    H256::repeat_byte(0xa),
                ])
                .build(),
            U64::from(100),
        );
        assert!(matched);
    }

    #[test]
    fn test_filter_combination_rejects_if_any_criterion_invalid() {
        let filter = LogFilter {
            from_block: BlockNumber::Number(U64::from(2)),
            to_block: BlockNumber::Number(U64::from(5)),
            addresses: vec![H160::repeat_byte(0xab)],
            topics: [Some(hashset! { H256::repeat_byte(0x1) }), None, None, None],
            updates: Default::default(),
        };

        let matched = filter.matches(
            &LogBuilder::new()
                .set_block(U64::from(1)) // invalid
                .set_address(H160::repeat_byte(0xab))
                .set_topics(vec![
                    H256::zero(),
                    H256::repeat_byte(0x1),
                    H256::repeat_byte(0x3),
                    H256::repeat_byte(0xa),
                ])
                .build(),
            U64::from(100),
        );
        assert!(!matched, "erroneously matched on invalid block number");

        let matched = filter.matches(
            &LogBuilder::new()
                .set_block(U64::from(1))
                .set_address(H160::repeat_byte(0xde)) // invalid
                .set_topics(vec![
                    H256::zero(),
                    H256::repeat_byte(0x1),
                    H256::repeat_byte(0x3),
                    H256::repeat_byte(0xa),
                ])
                .build(),
            U64::from(100),
        );
        assert!(!matched, "erroneously matched on invalid address");

        let matched = filter.matches(
            &LogBuilder::new()
                .set_block(U64::from(1))
                .set_address(H160::repeat_byte(0xde))
                .set_topics(vec![H256::zero(), H256::zero(), H256::zero(), H256::zero()]) // invalid
                .build(),
            U64::from(100),
        );
        assert!(!matched, "erroneously matched on invalid topic");
    }
}
