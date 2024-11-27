use anyhow::anyhow;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Shared readable view on time.
pub trait TimeRead {
    /// Returns the last timestamp (in seconds) that has already been used.
    fn last_timestamp(&self) -> u64;

    /// Peek at what the next call to `next_timestamp` will return.
    fn peek_next_timestamp(&self) -> u64;
}

/// Writeable view on time management. The owner of this view should be able to treat it as
/// exclusive access to the underlying clock.
pub trait TimeExclusive: TimeRead {
    /// Returns the next unique timestamp in seconds.
    ///
    /// Subsequent calls to this method return monotonically increasing values.
    fn next_timestamp(&mut self) -> u64;
}

/// Manages timestamps (in seconds) across the system.
///
/// Clones always agree on the underlying timestamp and updating one affects all other instances.
#[derive(Clone, Debug, Default)]
pub struct TimestampManager {
    internal: Arc<RwLock<TimestampManagerInternal>>,
}

impl TimestampManager {
    pub fn new(last_timestamp: u64) -> TimestampManager {
        TimestampManager {
            internal: Arc::new(RwLock::new(TimestampManagerInternal {
                last_timestamp,
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
    pub fn set_last_timestamp_unchecked(&self, timestamp: u64) -> i128 {
        let mut this = self.get_mut();
        let diff = (timestamp as i128).saturating_sub(this.last_timestamp as i128);
        this.reset_to(timestamp);
        diff
    }

    /// Forces clock to return provided value as the next timestamp. Time skip will not be performed
    /// before the next invocation of `next_timestamp`.
    ///
    /// Expects provided timestamp to be in the future, returns error otherwise.
    pub fn enforce_next_timestamp(&self, timestamp: u64) -> anyhow::Result<()> {
        let mut this = self.get_mut();
        if timestamp <= this.last_timestamp {
            Err(anyhow!(
                "timestamp ({}) must be greater than the last used timestamp ({})",
                timestamp,
                this.last_timestamp
            ))
        } else {
            this.next_timestamp.replace(timestamp);
            Ok(())
        }
    }

    /// Fast-forwards time by the given amount of seconds.
    pub fn increase_time(&self, seconds: u64) -> u64 {
        let mut this = self.get_mut();
        let next = this.last_timestamp.saturating_add(seconds);
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
    pub fn lock(&self) -> impl TimeExclusive + '_ {
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
    ) -> impl TimeExclusive + 'a
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

impl TimeRead for TimestampManager {
    fn last_timestamp(&self) -> u64 {
        (*self.get()).last_timestamp()
    }

    fn peek_next_timestamp(&self) -> u64 {
        (*self.get()).peek_next_timestamp()
    }
}

#[derive(Debug, Default)]
struct TimestampManagerInternal {
    /// The latest timestamp (in seconds) that has already been used.
    last_timestamp: u64,
    /// The next timestamp (in seconds) that the clock will be forced to return.
    next_timestamp: Option<u64>,
    /// The interval to use when determining the next block's timestamp (i.e. the difference in
    /// seconds between two consecutive blocks).
    interval: Option<u64>,
}

impl TimestampManagerInternal {
    fn reset_to(&mut self, timestamp: u64) {
        self.next_timestamp.take();
        self.last_timestamp = timestamp;
    }

    fn interval(&self) -> u64 {
        self.interval.unwrap_or(1)
    }
}

impl TimeRead for TimestampManagerInternal {
    fn last_timestamp(&self) -> u64 {
        self.last_timestamp
    }

    fn peek_next_timestamp(&self) -> u64 {
        self.next_timestamp
            .unwrap_or_else(|| self.last_timestamp.saturating_add(self.interval()))
    }
}

impl TimeExclusive for TimestampManagerInternal {
    fn next_timestamp(&mut self) -> u64 {
        let next_timestamp = match self.next_timestamp.take() {
            Some(next_timestamp) => next_timestamp,
            None => self.last_timestamp.saturating_add(self.interval()),
        };

        self.last_timestamp = next_timestamp;
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

impl TimeRead for TimeLockWithOffsets<'_> {
    fn last_timestamp(&self) -> u64 {
        self.guard.last_timestamp()
    }

    fn peek_next_timestamp(&self) -> u64 {
        match self.offsets.front() {
            Some(offset) => self.start_timestamp.saturating_add(*offset),
            None => self.guard.peek_next_timestamp(),
        }
    }
}

impl TimeExclusive for TimeLockWithOffsets<'_> {
    fn next_timestamp(&mut self) -> u64 {
        match self.offsets.pop_front() {
            Some(offset) => {
                let timestamp = self.start_timestamp.saturating_add(offset);
                // Persist last used timestamp in the underlying state as this instance can be
                // dropped before we finish iterating all values.
                self.guard.reset_to(timestamp);

                timestamp
            }
            None => self.guard.next_timestamp(),
        }
    }
}
