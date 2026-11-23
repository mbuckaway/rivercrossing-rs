// SPDX-License-Identifier: GPL-3.0-only

//! The shoe: a seeded, shuffled multi-deck stack with restitution and replays.

use crate::card::{Card, Rank, Suit};
use crate::error::CardsError;
use crate::rng::CpRandom;

/// The number of cards in one deck.
pub(crate) const DECK_SIZE: usize = 52;
/// The largest allowed number of jokers per deck.
const MAX_JOKERS_PER_DECK: u8 = 10;

/// How a shoe counts its jokers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JokersMode {
    /// Each deck carries `jokers_per_deck` jokers.
    PerDeck,
    /// The whole shoe carries `jokers_per_deck` jokers in total.
    Total,
}

/// Return an unshuffled stack of natural cards, deck-major, then rank, then suit.
pub(crate) fn natural_deck(decks: u8) -> Vec<Card> {
    let mut cards = Vec::with_capacity(usize::from(decks) * DECK_SIZE);
    for _ in 0..decks {
        for rank in Rank::ALL {
            for suit in Suit::ALL {
                cards.push(Card::Natural { rank, suit });
            }
        }
    }
    cards
}

/// Return the unshuffled stack for one cycle of the shoe.
fn build_cards(decks: u8, jokers_per_deck: u8, mode: JokersMode) -> Vec<Card> {
    match mode {
        JokersMode::PerDeck => {
            let mut cards = Vec::new();
            for _ in 0..decks {
                cards.extend(natural_deck(1));
                cards.extend(std::iter::repeat_n(
                    Card::Joker,
                    usize::from(jokers_per_deck),
                ));
            }
            cards
        }
        JokersMode::Total => {
            let mut cards = natural_deck(decks);
            cards.extend(std::iter::repeat_n(
                Card::Joker,
                usize::from(jokers_per_deck),
            ));
            cards
        }
    }
}

/// A seeded shoe of one or more decks.
#[derive(Debug)]
pub struct Shoe {
    decks: u8,
    jokers_per_deck: u8,
    seed: u64,
    cycle: u32,
    mode: JokersMode,
    cards: Vec<Card>,
    dealt: usize,
    joker_budget: u8,
    closed: bool,
}

impl Shoe {
    /// Create a shuffled shoe for `decks` decks under `seed`.
    ///
    /// A shoe starts open on cycle 1.
    ///
    /// # Errors
    ///
    /// Return [`CardsError::InvalidConfig`] when `decks` is zero or `jokers_per_deck`
    /// is above 10.
    #[expect(
        clippy::too_many_arguments,
        reason = "the four arguments are the documented shoe configuration"
    )]
    pub fn new(
        decks: u8,
        jokers_per_deck: u8,
        seed: u64,
        mode: JokersMode,
    ) -> Result<Self, CardsError> {
        if decks == 0 || jokers_per_deck > MAX_JOKERS_PER_DECK {
            return Err(CardsError::InvalidConfig {
                decks,
                jokers_per_deck,
            });
        }
        let mut cards = build_cards(decks, jokers_per_deck, mode);
        CpRandom::new(seed).shuffle(&mut cards);
        Ok(Self {
            decks,
            jokers_per_deck,
            seed,
            cycle: 1,
            mode,
            cards,
            dealt: 0,
            joker_budget: jokers_per_deck,
            closed: false,
        })
    }

    /// Deal the next card and its zero-based deal index.
    ///
    /// # Errors
    ///
    /// Return [`CardsError::ShoeClosed`] when the shoe is closed, and
    /// [`CardsError::ShoeEmpty`] when every card of the cycle is dealt.
    pub fn deal(&mut self) -> Result<(Card, usize), CardsError> {
        if self.closed {
            return Err(CardsError::ShoeClosed);
        }
        let index = self.dealt;
        let Some(card) = self.cards.get(index).cloned() else {
            return Err(CardsError::ShoeEmpty);
        };
        self.dealt += 1;
        if self.mode == JokersMode::Total && card.is_joker() {
            self.joker_budget = self.joker_budget.saturating_sub(1);
        }
        Ok((card, index))
    }

    /// Return the last dealt card to the shoe.
    ///
    /// # Errors
    ///
    /// Return [`CardsError::ShoeClosed`] when the shoe is closed, and
    /// [`CardsError::NotLastDealt`] when `card` is not the last card dealt.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the public API takes the card by value, as the oracle's restitute(card) does"
    )]
    pub fn restitute(&mut self, card: Card) -> Result<(), CardsError> {
        if self.closed {
            return Err(CardsError::ShoeClosed);
        }
        let last = self
            .dealt
            .checked_sub(1)
            .and_then(|index| self.cards.get(index));
        if last != Some(&card) {
            return Err(CardsError::NotLastDealt { code: card.code() });
        }
        self.dealt -= 1;
        if self.mode == JokersMode::Total && card.is_joker() {
            self.joker_budget = self.joker_budget.saturating_add(1);
        }
        Ok(())
    }

    /// Rebuild and reshuffle the shoe for the next cycle.
    ///
    /// The stored seed never changes; cycle `n` shuffles under `seed + n - 1`.
    ///
    /// # Errors
    ///
    /// Return [`CardsError::ShoeClosed`] when the shoe is closed.
    pub fn reshuffle(&mut self) -> Result<(), CardsError> {
        if self.closed {
            return Err(CardsError::ShoeClosed);
        }
        let offset = u64::from(self.cycle);
        self.cycle = self.cycle.wrapping_add(1);
        let mut cards = build_cards(self.decks, self.cycle_jokers(), self.mode);
        CpRandom::new(self.seed.wrapping_add(offset)).shuffle(&mut cards);
        self.cards = cards;
        self.dealt = 0;
        Ok(())
    }

    /// Rebuild cycle 1 from the stored seed under new deck settings.
    ///
    /// The rebuild reshuffles the same stored seed with the new settings, so
    /// the edited configuration deals the cards a fresh shoe of that shape
    /// would deal. The ride config bounds both counts before this call.
    ///
    /// # Errors
    ///
    /// Return [`CardsError::ShoeClosed`] when the shoe is closed.
    #[expect(
        clippy::too_many_arguments,
        reason = "the four arguments are the documented shoe configuration"
    )]
    pub fn reconfigure(
        &mut self,
        decks: u8,
        jokers_per_deck: u8,
        mode: JokersMode,
    ) -> Result<(), CardsError> {
        if self.closed {
            return Err(CardsError::ShoeClosed);
        }
        self.decks = decks;
        self.jokers_per_deck = jokers_per_deck;
        self.mode = mode;
        self.joker_budget = jokers_per_deck;
        self.cycle = 1;
        self.dealt = 0;
        let mut cards = build_cards(decks, jokers_per_deck, mode);
        CpRandom::new(self.seed).shuffle(&mut cards);
        self.cards = cards;
        Ok(())
    }

    /// Close the shoe and refuse further dealing.
    pub fn close(&mut self) {
        self.closed = true;
    }

    /// Reopen the shoe where it stood closed.
    pub fn reopen(&mut self) {
        self.closed = false;
    }

    /// Return a fresh open shoe after `cycles - 1` drained cycles and `deals` deals.
    ///
    /// # Errors
    ///
    /// Return [`CardsError::InvalidConfig`] for a bad configuration, and
    /// [`CardsError::ShoeEmpty`] when `deals` is past the end of the cycle.
    #[expect(
        clippy::too_many_arguments,
        reason = "the six arguments are the documented replay configuration"
    )]
    pub fn replay(
        decks: u8,
        jokers_per_deck: u8,
        seed: u64,
        deals: usize,
        cycles: u32,
        mode: JokersMode,
    ) -> Result<Self, CardsError> {
        let mut shoe = Self::new(decks, jokers_per_deck, seed, mode)?;
        for _ in 1..cycles {
            shoe.drain()?;
            shoe.reshuffle()?;
        }
        for _ in 0..deals {
            shoe.deal()?;
        }
        Ok(shoe)
    }

    /// Return the number of cards left in the cycle.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.cards.len() - self.dealt
    }

    /// Return the number of cards dealt from the cycle.
    #[must_use]
    pub fn dealt(&self) -> usize {
        self.dealt
    }

    /// Return the current cycle number, which starts at 1.
    #[must_use]
    pub fn cycle(&self) -> u32 {
        self.cycle
    }

    /// Return the stored seed. A reshuffle never changes it.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Return the number of jokers in the current cycle.
    #[must_use]
    pub fn jokers_in_cycle(&self) -> u8 {
        match self.mode {
            JokersMode::PerDeck => self.decks.saturating_mul(self.jokers_per_deck),
            JokersMode::Total => self.joker_budget,
        }
    }

    /// Return whether the shoe is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Return the joker count that the next cycle is built with.
    fn cycle_jokers(&self) -> u8 {
        match self.mode {
            JokersMode::PerDeck => self.jokers_per_deck,
            JokersMode::Total => self.joker_budget,
        }
    }

    /// Deal every remaining card of the cycle.
    fn drain(&mut self) -> Result<(), CardsError> {
        for _ in 0..self.remaining() {
            self.deal()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
