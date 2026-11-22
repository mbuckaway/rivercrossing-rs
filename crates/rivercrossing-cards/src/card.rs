// SPDX-License-Identifier: GPL-3.0-only

//! The card model: the suit, the rank, and their code text.

use serde::{Deserialize, Serialize};

use crate::error::CardsError;

/// The code of a joker.
const JOKER_CODE: &str = "JK";

/// A card suit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Suit {
    /// Clubs.
    #[serde(rename = "c")]
    Clubs,
    /// Diamonds.
    #[serde(rename = "d")]
    Diamonds,
    /// Hearts.
    #[serde(rename = "h")]
    Hearts,
    /// Spades.
    #[serde(rename = "s")]
    Spades,
}

impl Suit {
    /// The four suits in draw-key order: clubs, diamonds, hearts, spades.
    pub(crate) const ALL: [Self; 4] = [Self::Clubs, Self::Diamonds, Self::Hearts, Self::Spades];

    /// Return the code letter of this suit.
    #[must_use]
    pub(crate) const fn letter(self) -> char {
        match self {
            Self::Clubs => 'C',
            Self::Diamonds => 'D',
            Self::Hearts => 'H',
            Self::Spades => 'S',
        }
    }

    /// Return the draw-key slot of this suit, with clubs as 0.
    #[must_use]
    pub(crate) const fn slot(self) -> u8 {
        match self {
            Self::Clubs => 0,
            Self::Diamonds => 1,
            Self::Hearts => 2,
            Self::Spades => 3,
        }
    }

    /// Return the suit of a code letter, or `None` when the letter is unknown.
    #[must_use]
    pub(crate) const fn from_letter(letter: char) -> Option<Self> {
        match letter {
            'C' => Some(Self::Clubs),
            'D' => Some(Self::Diamonds),
            'H' => Some(Self::Hearts),
            'S' => Some(Self::Spades),
            _ => None,
        }
    }
}

/// A card rank, with the pip value as its discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
#[repr(u8)]
pub enum Rank {
    /// Two.
    Two = 2,
    /// Three.
    Three = 3,
    /// Four.
    Four = 4,
    /// Five.
    Five = 5,
    /// Six.
    Six = 6,
    /// Seven.
    Seven = 7,
    /// Eight.
    Eight = 8,
    /// Nine.
    Nine = 9,
    /// Ten.
    Ten = 10,
    /// Jack.
    Jack = 11,
    /// Queen.
    Queen = 12,
    /// King.
    King = 13,
    /// Ace.
    Ace = 14,
}

impl Rank {
    /// The thirteen ranks from two to ace.
    pub(crate) const ALL: [Self; 13] = [
        Self::Two,
        Self::Three,
        Self::Four,
        Self::Five,
        Self::Six,
        Self::Seven,
        Self::Eight,
        Self::Nine,
        Self::Ten,
        Self::Jack,
        Self::Queen,
        Self::King,
        Self::Ace,
    ];

    /// Return the pip value of this rank, two as 2 and ace as 14.
    #[must_use]
    pub const fn value(self) -> u8 {
        self as u8
    }

    /// Return the rank of a pip value.
    ///
    /// # Errors
    ///
    /// Return [`CardsError::InvalidCode`] when `value` is not in `2..=14`.
    pub fn from_u8(value: u8) -> Result<Self, CardsError> {
        Self::ALL
            .into_iter()
            .find(|rank| rank.value() == value)
            .ok_or_else(|| CardsError::InvalidCode {
                code: value.to_string(),
            })
    }

    /// Return the code token of this rank.
    #[must_use]
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Two => "2",
            Self::Three => "3",
            Self::Four => "4",
            Self::Five => "5",
            Self::Six => "6",
            Self::Seven => "7",
            Self::Eight => "8",
            Self::Nine => "9",
            Self::Ten => "10",
            Self::Jack => "J",
            Self::Queen => "Q",
            Self::King => "K",
            Self::Ace => "A",
        }
    }

    /// Return the rank of a code token, or `None` when the token is unknown.
    #[must_use]
    pub(crate) fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|rank| rank.token() == token)
    }
}

impl From<Rank> for u8 {
    fn from(rank: Rank) -> Self {
        rank.value()
    }
}

impl TryFrom<u8> for Rank {
    type Error = CardsError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::from_u8(value)
    }
}

/// A card: a natural card with one rank and one suit, or a joker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Card {
    /// A natural card.
    Natural {
        /// The rank of the card.
        rank: Rank,
        /// The suit of the card.
        suit: Suit,
    },
    /// A joker.
    Joker,
}

impl Card {
    /// Return the code text of this card, such as `10C` or `JK`.
    #[must_use]
    pub fn code(&self) -> String {
        match self {
            Self::Natural { rank, suit } => format!("{}{}", rank.token(), suit.letter()),
            Self::Joker => JOKER_CODE.to_owned(),
        }
    }

    /// Parse a card code, such as `10C` or `JK`.
    ///
    /// The last character is the suit letter and the prefix is the rank token.
    ///
    /// # Errors
    ///
    /// Return [`CardsError::InvalidCode`] when the text is empty, when the last character
    /// is not a suit letter, or when the prefix is not a rank token.
    pub fn parse(code: &str) -> Result<Self, CardsError> {
        if code == JOKER_CODE {
            return Ok(Self::Joker);
        }
        let mut characters = code.chars();
        let Some(letter) = characters.next_back() else {
            return Err(CardsError::InvalidCode {
                code: code.to_owned(),
            });
        };
        let Some(suit) = Suit::from_letter(letter) else {
            return Err(CardsError::InvalidCode {
                code: code.to_owned(),
            });
        };
        let Some(rank) = Rank::from_token(characters.as_str()) else {
            return Err(CardsError::InvalidCode {
                code: code.to_owned(),
            });
        };
        Ok(Self::Natural { rank, suit })
    }

    /// Return whether this card is a joker.
    #[must_use]
    pub const fn is_joker(&self) -> bool {
        matches!(self, Self::Joker)
    }
}

impl TryFrom<String> for Card {
    type Error = CardsError;

    fn try_from(code: String) -> Result<Self, Self::Error> {
        Self::parse(&code)
    }
}

impl From<Card> for String {
    fn from(card: Card) -> Self {
        card.code()
    }
}

#[cfg(test)]
mod tests;
