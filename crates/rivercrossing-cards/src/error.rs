// SPDX-License-Identifier: GPL-3.0-only

//! The error type of the card crate.

use thiserror::Error;

/// A failure from the card model, the shoe, or a draw helper.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CardsError {
    /// The shoe has no card left to deal.
    #[error("shoe is empty")]
    ShoeEmpty,
    /// The shoe is closed.
    #[error("shoe is closed")]
    ShoeClosed,
    /// A restitution named a card that was not the last one dealt.
    #[error("{code} was not the last card dealt from this shoe")]
    NotLastDealt {
        /// The code of the card that the caller tried to return.
        code: String,
    },
    /// A text is not a card code.
    #[error("{code} is not a valid card code")]
    InvalidCode {
        /// The text that failed to parse.
        code: String,
    },
    /// A joker has no draw key.
    #[error("a joker has no draw key")]
    JokerHasNoDrawKey,
    /// The shoe configuration is outside the allowed range.
    #[error("invalid shoe config: {decks} decks, {jokers_per_deck} jokers per deck")]
    InvalidConfig {
        /// The requested deck count.
        decks: u8,
        /// The requested joker count.
        jokers_per_deck: u8,
    },
}

#[cfg(test)]
mod tests;
