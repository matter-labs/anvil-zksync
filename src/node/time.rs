use anyhow::anyhow;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Shared readable view on time.
pub trait ReadTime {
    /// Returns timestamp (in seconds) that the clock is currently on.
    fn current_timestamp(&self) -> u64;

    /// Peek at what the next call to `advance_timestamp` will return.
    fn peek_next_timestamp(&self) -> u64;
}

/// Writeable view on time management. The owner of this view should be able to treat it as
/// exclusive access to the underlying clock.
pub trait AdvanceTime: ReadTime {
    /// Advances clock to the next timestamp and returns that timestamp in seconds.
    ///
    /// Subsequent calls to this method return monotonically increasing values. Time difference
    /// between calls is implementation-specific.
    fn advance_timestamp(&mut self) -> u64;
}

/// Manages timestamps (in seconds) across the system.
///
/// Clones always agree on the underlying timestamp and updating one affects all other instances.
#[derive(Clone, Debug, Default)]
pub struct TimestampManager {
    internal: Arc<RwLock<TimestampManagerInternal>>,
}

impl TimestampManager {
    pub fn new(current_timestamp: u64) -> TimestampManager {
        TimestampManager {
            internal: Arc::new(RwLock::new(TimestampManagerInternal {
                current_timestamp,
                next_timestamp: None,
                interval: None,
            })),
        }
    }

    fn get(&self) -> RwLockReadGuard<TimestampManagerInternal> {
        self.internal
            .read()
            .expect("TimestampManager lock is poisoned")
    }

    fn get_mut(&self) -> RwLockWriteGuard<TimestampManagerInternal> {
        self.internal
            .write()
            .expect("TimestampManager lock is poisoned")
    }

    /// Sets last used timestamp (in seconds) to the provided value and returns the difference
    /// between new value and old value (represented as a signed number of seconds).
    pub fn set_current_timestamp_unchecked(&self, timestamp: u64) -> i128 {
        let mut this = self.get_mut();
        let diff = (timestamp as i128).saturating_sub(this.current_timestamp as i128);
        this.reset_to(timestamp);
        diff
    }

    /// Forces clock to return provided value as the next timestamp. Time skip will not be performed
    /// before the next invocation of `advance_timestamp`.
    ///
    /// Expects provided timestamp to be in the future, returns error otherwise.
    pub fn enforce_next_timestamp(&self, timestamp: u64) -> anyhow::Result<()> {
        let mut this = self.get_mut();
        if timestamp <= this.current_timestamp {
            Err(anyhow!(
                "timestamp ({}) must be greater than the last used timestamp ({})",
                timestamp,
                this.current_timestamp
            ))
        } else {
            this.next_timestamp.replace(timestamp);
            Ok(())
        }
    }

    /// Fast-forwards time by the given amount of seconds.
    pub fn increase_time(&self, seconds: u64) -> u64 {
        let mut this = self.get_mut();
        let next = this.current_timestamp.saturating_add(seconds);
        this.reset_to(next);
        next
    }

    /// Sets an interval to use when computing the next timestamp
    ///
    /// If an interval already exists, this will update the interval, otherwise a new interval will
    /// be set starting with the current timestamp.
    pub fn set_block_timestamp_interval(&self, seconds: u64) {
        self.get_mut().interval.replace(seconds);
    }

    /// Removes the interval. Returns true if it existed before being removed, false otherwise.
    pub fn remove_block_timestamp_interval(&self) -> bool {
        self.get_mut().interval.take().is_some()
    }

    /// Returns an exclusively owned writeable view on this [`TimeManager`] instance.
    ///
    /// Use this method when you need to ensure that no one else can access [`TimeManager`] during
    /// this view's lifetime.
    pub fn lock(&self) -> impl AdvanceTime + '_ {
        self.lock_with_offsets([])
    }

    /// Returns an exclusively owned writeable view on this [`TimeManager`] instance where first N
    /// timestamps will be offset by the provided amount of seconds (where `N` is the size of
    /// iterator).
    ///
    /// Use this method when you need to ensure that no one else can access [`TimeManager`] during
    /// this view's lifetime while also pre-setting first `N` returned timestamps.
    pub fn lock_with_offsets<'a, I: IntoIterator<Item = u64>>(
        &'a self,
        offsets: I,
    ) -> impl AdvanceTime + 'a
    where
        <I as IntoIterator>::IntoIter: 'a,
    {
        let guard = self.get_mut();
        TimeLockWithOffsets {
            start_timestamp: guard.peek_next_timestamp(),
            guard,
            offsets: offsets.into_iter().collect::<VecDeque<_>>(),
        }
    }
}

impl ReadTime for TimestampManager {
    fn current_timestamp(&self) -> u64 {
        (*self.get()).current_timestamp()
    }

    fn peek_next_timestamp(&self) -> u64 {
        (*self.get()).peek_next_timestamp()
    }
}

#[derive(Debug, Default)]
struct TimestampManagerInternal {
    /// The current timestamp (in seconds). This timestamp is considered to be used already: there
    /// might be a logical event that already happened on that timestamp (e.g. a block was sealed
    /// with this timestamp).
    current_timestamp: u64,
    /// The next timestamp (in seconds) that the clock will be forced to advance to.
    next_timestamp: Option<u64>,
    /// The interval to use when determining the next timestamp to advance to.
    interval: Option<u64>,
}

impl TimestampManagerInternal {
    fn reset_to(&mut self, timestamp: u64) {
        self.next_timestamp.take();
        self.current_timestamp = timestamp;
    }

    fn interval(&self) -> u64 {
        self.interval.unwrap_or(1)
    }
}

impl ReadTime for TimestampManagerInternal {
    fn current_timestamp(&self) -> u64 {
        self.current_timestamp
    }

    fn peek_next_timestamp(&self) -> u64 {
        self.next_timestamp
            .unwrap_or_else(|| self.current_timestamp.saturating_add(self.interval()))
    }
}

impl AdvanceTime for TimestampManagerInternal {
    fn advance_timestamp(&mut self) -> u64 {
        let next_timestamp = match self.next_timestamp.take() {
            Some(next_timestamp) => next_timestamp,
            None => self.current_timestamp.saturating_add(self.interval()),
        };

        self.current_timestamp = next_timestamp;
        next_timestamp
    }
}

struct TimeLockWithOffsets<'a> {
    /// The first timestamp that would have been returned without accounting for offsets
    start_timestamp: u64,
    /// Exclusive writable ownership over the corresponding [`TimestampManager`]
    guard: RwLockWriteGuard<'a, TimestampManagerInternal>,
    /// A queue of offsets (relative to `start_timestamp`) to be used for next `N` timestamps
    offsets: VecDeque<u64>,
}

impl ReadTime for TimeLockWithOffsets<'_> {
    fn current_timestamp(&self) -> u64 {
        self.guard.current_timestamp()
    }

    fn peek_next_timestamp(&self) -> u64 {
        match self.offsets.front() {
            Some(offset) => self.start_timestamp.saturating_add(*offset),
            None => self.guard.peek_next_timestamp(),
        }
    }
}

impl AdvanceTime for TimeLockWithOffsets<'_> {
    fn advance_timestamp(&mut self) -> u64 {
        match self.offsets.pop_front() {
            Some(offset) => {
                let timestamp = self.start_timestamp.saturating_add(offset);
                // Persist last used timestamp in the underlying state as this instance can be
                // dropped before we finish iterating all values.
                self.guard.reset_to(timestamp);

                timestamp
            }
            None => self.guard.advance_timestamp(),
        }
    }
}
