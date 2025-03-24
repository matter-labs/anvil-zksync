use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use zksync_error::anvil_zksync::{self, node::AnvilNodeError};

/// Read-only view on time.
pub trait ReadTime: Send + Sync {
    /// Alternative for [`Clone::clone`] that is object safe.
    fn dyn_cloned(&self) -> Box<dyn ReadTime>;

    /// Returns timestamp (in seconds) that the clock is currently on.
    fn current_timestamp(&self) -> u64;
}

impl Clone for Box<dyn ReadTime> {
    fn clone(&self) -> Self {
        self.dyn_cloned()
    }
}

#[derive(Debug, Clone)]
pub(super) struct Time {
    internal: Arc<RwLock<TimeState>>,
}

impl ReadTime for Time {
    fn dyn_cloned(&self) -> Box<dyn ReadTime> {
        Box::new(self.clone())
    }

    fn current_timestamp(&self) -> u64 {
        self.get().current_timestamp
    }
}

impl Time {
    pub(super) fn new(current_timestamp: u64) -> Self {
        let internal = Arc::new(RwLock::new(TimeState {
            current_timestamp,
            next_timestamp: None,
            interval: None,
        }));

        Self { internal }
    }

    fn get(&self) -> RwLockReadGuard<TimeState> {
        self.internal
            .read()
            .expect("TimestampWriter lock is poisoned")
    }

    fn get_mut(&self) -> RwLockWriteGuard<TimeState> {
        self.internal
            .write()
            .expect("TimestampWriter lock is poisoned")
    }

    /// Sets last used timestamp (in seconds) to the provided value and returns the difference
    /// between new value and old value (represented as a signed number of seconds).
    pub(super) fn set_current_timestamp_unchecked(&self, timestamp: u64) -> i128 {
        let mut this = self.get_mut();
        let diff = (timestamp as i128).saturating_sub(this.current_timestamp as i128);
        this.next_timestamp.take();
        this.current_timestamp = timestamp;
        diff
    }

    /// Forces clock to return provided value as the next timestamp. Time skip will not be performed
    /// before the next invocation of `advance_timestamp`.
    ///
    /// Expects provided timestamp to be in the future, returns error otherwise.
    pub(super) fn enforce_next_timestamp(&self, timestamp: u64) -> Result<(), AnvilNodeError> {
        let mut this = self.get_mut();
        if timestamp <= this.current_timestamp {
            Err(anvil_zksync::node::TimestampBackwardsError {
                timestamp_requested: timestamp.into(),
                timestamp_now: this.current_timestamp.into(),
            })
        } else {
            this.next_timestamp.replace(timestamp);
            Ok(())
        }
    }

    /// Fast-forwards time by the given amount of seconds.
    pub(super) fn increase_time(&self, seconds: u64) -> u64 {
        let mut this = self.get_mut();
        let next = this.current_timestamp.saturating_add(seconds);
        this.next_timestamp.take();
        this.current_timestamp = next;
        next
    }

    pub(super) fn get_block_timestamp_interval(&self) -> Option<u64> {
        self.get().interval
    }

    /// Sets an interval to use when computing the next timestamp
    ///
    /// If an interval already exists, this will update the interval, otherwise a new interval will
    /// be set starting with the current timestamp.
    pub(super) fn set_block_timestamp_interval(&self, seconds: Option<u64>) {
        self.get_mut().interval = seconds;
    }

    /// Removes the interval. Returns true if it existed before being removed, false otherwise.
    pub(super) fn remove_block_timestamp_interval(&self) -> bool {
        self.get_mut().interval.take().is_some()
    }

    /// Peek at what the next call to `advance_timestamp` will return.
    pub(super) fn peek_next_timestamp(&self) -> u64 {
        let internal = self.get();
        internal.next_timestamp.unwrap_or_else(|| {
            internal
                .current_timestamp
                .saturating_add(internal.interval())
        })
    }

    /// Advances clock to the next timestamp and returns that timestamp in seconds.
    ///
    /// Subsequent calls to this method return monotonically increasing values. Time difference
    /// between calls is implementation-specific.
    pub(super) fn advance_timestamp(&self) -> u64 {
        let mut internal = self.get_mut();
        let next_timestamp = match internal.next_timestamp.take() {
            Some(next_timestamp) => next_timestamp,
            None => internal
                .current_timestamp
                .saturating_add(internal.interval()),
        };

        internal.current_timestamp = next_timestamp;
        next_timestamp
    }

    /// Reset current timestamp to the provided value. WARNING: Moving clock to the past can cause
    /// unexpected behavior.
    pub(super) fn reset_to(&self, timestamp: u64) {
        let mut internal = self.get_mut();
        internal.next_timestamp.take();
        internal.current_timestamp = timestamp;
    }
}

#[derive(Debug, Default)]
struct TimeState {
    /// The current timestamp (in seconds). This timestamp is considered to be used already: there
    /// might be a logical event that already happened on that timestamp (e.g. a block was sealed
    /// with this timestamp).
    current_timestamp: u64,
    /// The next timestamp (in seconds) that the clock will be forced to advance to.
    next_timestamp: Option<u64>,
    /// The interval to use when determining the next timestamp to advance to.
    interval: Option<u64>,
}

impl TimeState {
    fn interval(&self) -> u64 {
        self.interval.unwrap_or(1)
    }
}
