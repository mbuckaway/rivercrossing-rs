// SPDX-License-Identifier: GPL-3.0-only

//! The wall-clock seam that the ride engine reads the current instant from.

use time::OffsetDateTime;

/// One instant on the wall clock, in UTC.
pub type Timestamp = OffsetDateTime;

/// The clock that the ride engine reads.
pub trait Clock {
    /// Return the current instant.
    fn now(&self) -> Timestamp;
}

/// The clock that reads the system time.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        OffsetDateTime::now_utc()
    }
}

#[cfg(test)]
mod tests;
