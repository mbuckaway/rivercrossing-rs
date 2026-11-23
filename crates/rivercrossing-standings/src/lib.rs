// SPDX-License-Identifier: GPL-3.0-only

//! Ride standings: ranking, tie-breaks, leaderboards and hand-name copy.
//!
//! The crate orders a finished ride's `SnapshotEntry` rows through its own
//! [`EntryResult`], never re-deriving a hand from the cards. The ride crate
//! owns the tie-break vocabulary, so [`TieBreak`] is re-exported here and a
//! stored order needs no conversion.

#![forbid(unsafe_code)]

pub mod boards;
pub mod enums;
pub mod error;
pub mod hand_name;
pub mod rank;
pub mod types;

pub use enums::ZeroLapMode;
pub use error::StandingsError;
pub use rivercrossing_ride::TieBreak;
pub use types::{EntryResult, Placed};

/// The venue's default tie-break order, most important first (R-14).
///
/// The high-card draw leads, so an undrawn hand tie stops there and flags for
/// the venue's draw while a drawn one is ordered by it.
pub const DEFAULT_TIEBREAK_ORDER: [TieBreak; 3] =
    [TieBreak::HighCard, TieBreak::Laps, TieBreak::TotalTime];

/// The live board's order: most laps, then shortest total time.
///
/// A live ride has no draw card yet, so the live board never reads one.
pub const LIVE_TIEBREAK_ORDER: [TieBreak; 2] = [TieBreak::Laps, TieBreak::TotalTime];

/// The note that marks a residual tie that no criterion resolved.
pub const DRAW_TIE_NOTE: &str = "draw required";

/// The default leaderboard length (R-60).
pub const DEFAULT_TOP: usize = 10;

/// Convert a stored ride tie-break order onto [`TieBreak`] members.
///
/// An empty slice is valid: a ride with no criteria, where every hand tie is a
/// draw.
///
/// # Errors
///
/// Return [`StandingsError::UnknownTieBreakSpelling`] when a spelling names no
/// criterion.
pub fn tiebreak_order_from_spellings(spellings: &[&str]) -> Result<Vec<TieBreak>, StandingsError> {
    spellings
        .iter()
        .map(|spelling| tiebreak_from_spelling(spelling))
        .collect()
}

/// Read one stored tie-break spelling.
fn tiebreak_from_spelling(spelling: &str) -> Result<TieBreak, StandingsError> {
    match spelling {
        "high_card" => Ok(TieBreak::HighCard),
        "laps" => Ok(TieBreak::Laps),
        "total_time" => Ok(TieBreak::TotalTime),
        other => Err(StandingsError::UnknownTieBreakSpelling {
            spelling: other.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests;
