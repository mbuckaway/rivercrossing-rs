// SPDX-License-Identifier: GPL-3.0-only

//! The scoring enums of the standings crate.

/// How a 0-lap ACTIVE entry is treated on a finished ride.
///
/// A rider who recorded no crossing still finished the ride ACTIVE, so a
/// finished ride treats their entry by this mode. Nothing persists the choice;
/// the members are the whole interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZeroLapMode {
    /// Rank the entry normally on its empty hand, as a live board does.
    Rank,
    /// Split the entry off before the ranking and append it unplaced.
    Dns,
    /// Drop the entry outright.
    Hide,
}

#[cfg(test)]
mod tests;
