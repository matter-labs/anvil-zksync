use anyhow::anyhow;
use std::sync::{Arc, RwLock};

/// Manages timestamps (in seconds) across the system.
///
/// Clones always agree on the underlying timestamp and updating one affects all other instances.
#[derive(Clone, Debug)]
pub struct TimestampManager {
    /// The latest timestamp (in seconds) that has already been used.
    last_timestamp: Arc<RwLock<u64>>,
}

impl TimestampManager {
    pub fn new(last_timestamp: u64) -> TimestampManager {
        TimestampManager {
            last_timestamp: Arc::new(RwLock::new(last_timestamp)),
        }
    }

    /// Returns the last timestamp (in seconds) that has already been used.
    pub fn last_timestamp(&self) -> u64 {
        *self
            .last_timestamp
            .read()
            .expect("TimestampManager lock is poisoned")
    }

    /// Returns the next unique timestamp (in seconds) to be used.
    pub fn next_timestamp(&self) -> u64 {
        let mut guard = self
            .last_timestamp
            .write()
            .expect("TimestampManager lock is poisoned");
        let next_timestamp = *guard + 1;
        *guard = next_timestamp;

        next_timestamp
    }

    /// Sets last used timestamp (in seconds) to the provided value and returns the difference
    /// between new value and old value (represented as a signed number of seconds).
    pub fn set_last_timestamp_unchecked(&self, timestamp: u64) -> i128 {
        let mut guard = self
            .last_timestamp
            .write()
            .expect("TimestampManager lock is poisoned");
        let diff = (timestamp as i128).saturating_sub(*guard as i128);
        *guard = timestamp;
        diff
    }

    /// Advances internal timestamp (in seconds) to the provided value.
    ///
    /// Expects provided timestamp to be in the future, returns error otherwise.
    pub fn advance_timestamp(&self, timestamp: u64) -> anyhow::Result<()> {
        let mut guard = self
            .last_timestamp
            .write()
            .expect("TimestampManager lock is poisoned");
        if timestamp < *guard {
            Err(anyhow!(
                "timestamp ({}) must be greater or equal than current timestamp ({})",
                timestamp,
                *guard
            ))
        } else {
            *guard = timestamp;
            Ok(())
        }
    }

    /// Fast-forwards time by the given amount of seconds.
    pub fn increase_time(&self, seconds: u64) -> u64 {
        let mut guard = self
            .last_timestamp
            .write()
            .expect("TimestampManager lock is poisoned");
        let next = guard.saturating_add(seconds);
        *guard = next;
        next
    }
}
