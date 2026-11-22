// SPDX-License-Identifier: GPL-3.0-only

//! The seeded natural-card order used by the random-card helpers.

use crate::card::Card;
use crate::rng::CpRandom;
use crate::shoe::{DECK_SIZE, natural_deck};

/// Return the 52 natural cards in the seeded shuffle order.
///
/// The shuffle covers `decks` decks and each card keeps its first occurrence, so the
/// result always holds 52 unique natural cards and never a joker.
#[must_use]
pub fn seeded_card_codes(seed: u64, decks: u8) -> Vec<Card> {
    let mut cards = natural_deck(decks);
    CpRandom::new(seed).shuffle(&mut cards);
    let mut unique: Vec<Card> = Vec::with_capacity(DECK_SIZE);
    for card in cards {
        if !unique.contains(&card) {
            unique.push(card);
        }
    }
    unique
}

#[cfg(test)]
mod tests;
