// SPDX-License-Identifier: GPL-3.0-only

//! The error type that every hand-evaluation entry point returns.

/// A hand could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HandsError {
    /// A hand did not hold exactly five cards.
    #[error("a hand must have exactly 5 cards, got {count}")]
    InvalidCardCount {
        /// The number of cards that the caller supplied.
        count: usize,
    },
}

#[cfg(test)]
mod tests;
