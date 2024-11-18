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

    /// Sets last used timestamp (in seconds) to the provided value.
    pub fn set_last_timestamp(&self, timestamp: u64) {
        let mut guard = self
            .last_timestamp
            .write()
            .expect("TimestampManager lock is poisoned");
        *guard = timestamp;
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
