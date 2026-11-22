// SPDX-License-Identifier: GPL-3.0-only

//! The rank order of the poker hand classes that a hand can hold.

/// A five-card poker hand class, ordered from weakest to strongest.
///
/// The discriminants are the comparison ranks, starting at one, so the derived
/// `Ord` agrees with the standard poker order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum HandClass {
    /// No combination; the hand is ranked by its highest cards.
    HighCard = 1,
    /// Two cards of one rank.
    Pair,
    /// Two cards of one rank and two of another.
    TwoPair,
    /// Three cards of one rank.
    Trips,
    /// Five consecutive ranks.
    Straight,
    /// Five cards of one suit.
    Flush,
    /// Three cards of one rank and two of another.
    FullHouse,
    /// Four cards of one rank.
    Quads,
    /// Five consecutive ranks of one suit.
    StraightFlush,
    /// A straight flush to the ace.
    RoyalFlush,
    /// Five cards of one rank, which only a joker shoe can deal.
    FiveOfAKind,
}

impl HandClass {
    /// The eleven classes, weakest first.
    pub const ALL: [Self; 11] = [
        Self::HighCard,
        Self::Pair,
        Self::TwoPair,
        Self::Trips,
        Self::Straight,
        Self::Flush,
        Self::FullHouse,
        Self::Quads,
        Self::StraightFlush,
        Self::RoyalFlush,
        Self::FiveOfAKind,
    ];

    /// Return the golden-vector name of this class, such as `ROYAL_FLUSH`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::HighCard => "HIGH_CARD",
            Self::Pair => "PAIR",
            Self::TwoPair => "TWO_PAIR",
            Self::Trips => "TRIPS",
            Self::Straight => "STRAIGHT",
            Self::Flush => "FLUSH",
            Self::FullHouse => "FULL_HOUSE",
            Self::Quads => "QUADS",
            Self::StraightFlush => "STRAIGHT_FLUSH",
            Self::RoyalFlush => "ROYAL_FLUSH",
            Self::FiveOfAKind => "FIVE_OF_A_KIND",
        }
    }

    /// Return the class that a golden-vector name holds, or `None`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.name() == name)
    }
}

#[cfg(test)]
mod tests;
