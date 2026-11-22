// SPDX-License-Identifier: GPL-3.0-only

//! The draw helpers: the draw key and the seeded high-card draw.

use crate::card::Card;
use crate::error::CardsError;
use crate::rng::CpRandom;
use crate::shoe::natural_deck;

/// The number of suits, the multiplier from a rank to its draw-key range.
const SUITS_PER_RANK: u8 = 4;

/// Return the draw key of a natural card.
///
/// The key orders the ranks from two to ace, and the suits within a rank from clubs to
/// spades: `rank * 4 + suit_slot`. The lowest key is 8 and the highest is 59.
///
/// # Errors
///
/// Return [`CardsError::JokerHasNoDrawKey`] when the card is a joker.
pub fn draw_key(card: &Card) -> Result<u8, CardsError> {
    match card {
        Card::Joker => Err(CardsError::JokerHasNoDrawKey),
        Card::Natural { rank, suit } => Ok(rank.value() * SUITS_PER_RANK + suit.slot()),
    }
}

/// Return the first `count` cards of the one-deck natural shuffle under `seed`.
///
/// A `count` above 52 returns the whole deck. No joker is ever drawn.
#[must_use]
pub fn high_card_draw(seed: u64, count: usize) -> Vec<Card> {
    let mut cards = natural_deck(1);
    CpRandom::new(seed).shuffle(&mut cards);
    cards.truncate(count);
    cards
}

#[cfg(test)]
mod tests;
