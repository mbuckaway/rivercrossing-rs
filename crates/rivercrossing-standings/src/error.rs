// SPDX-License-Identifier: GPL-3.0-only

//! The error type of the standings crate.

use rivercrossing_hands::HandClass;

/// A standings refusal.
///
/// Each message keeps the Python oracle's wording, so an operator reads the
/// same sentence in the port as in the Python app.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StandingsError {
    /// A stored tie-break spelling names no criterion.
    #[error("unknown tie-break spelling: '{spelling}'")]
    UnknownTieBreakSpelling {
        /// The spelling that the caller supplied.
        spelling: String,
    },
    /// A hand holds no cards, so it has no name.
    #[error("cannot name an empty hand")]
    EmptyHand,
    /// A hand class arrived without its count.
    #[error("hand class {class:?} is missing a count")]
    MalformedHand {
        /// The class that is missing its count.
        class: HandClass,
    },
    /// A joker holds no draw key.
    #[error("a joker has no draw key")]
    JokerDrawKey,
}

#[cfg(test)]
mod tests;
