// SPDX-License-Identifier: GPL-3.0-only

//! The hand-name renderer that the results window and the exports reuse.
//!
//! The wording is the Python oracle's (`standings.py:hand_name`): one
//! title-case prose name per hand class, with U+2014 between the class and the
//! rank. A group class names the rank it holds most often, never a fixed group
//! size, because five identical naturals evaluate as four of a kind; a joker's
//! resolution supplies its rank, so a joker-completed hand names its true
//! kickers.

use rivercrossing_cards::Card;
use rivercrossing_hands::{EvaluatedHand, HandClass};

use crate::error::StandingsError;

/// The em dash that separates a hand's class from its rank, spaces included.
const DASH: &str = " — ";

/// The count of a pair.
const PAIR_COUNT: usize = 2;

/// The count of three of a kind.
const TRIPS_COUNT: usize = 3;

/// The lowest rank value, two.
const LOWEST_RANK: u8 = 2;

/// The number of rank values, two to ace.
const RANK_COUNT: usize = 13;

/// The rank values of the wheel; it plays five high, never ace high.
const WHEEL: [u8; 5] = [2, 3, 4, 5, 14];

/// The high card that the wheel straight plays.
const WHEEL_HIGH: u8 = 5;

/// The singular word of each rank value, two to ace.
///
/// English plurals are irregular (Fives, Sixes, Nines, Tens), so the two
/// tables are written out rather than derived from a rule.
const RANK_WORD: [&str; RANK_COUNT] = [
    "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten", "Jack", "Queen",
    "King", "Ace",
];

/// The plural word of each rank value, two to ace.
const RANK_PLURAL: [&str; RANK_COUNT] = [
    "Twos", "Threes", "Fours", "Fives", "Sixes", "Sevens", "Eights", "Nines", "Tens", "Jacks",
    "Queens", "Kings", "Aces",
];

/// Return *hand*'s prose name, such as `Four of a Kind — Nines`.
///
/// The name is the one em-dash form that the results window and the exports
/// share. A group class names the rank it holds most often (a count tie takes
/// the higher rank), a joker's resolution supplies its rank, and a hand of
/// fewer than five cards renders the class it makes with no marker.
///
/// # Errors
///
/// Return [`StandingsError::EmptyHand`] when the hand holds no cards, and
/// [`StandingsError::MalformedHand`] when a full house arrives without its
/// trips or pair count.
pub fn hand_name(hand: &EvaluatedHand) -> Result<String, StandingsError> {
    let ranks = effective_ranks(hand);
    if ranks.is_empty() {
        return Err(StandingsError::EmptyHand);
    }
    name_by_class(hand.cls(), &ranks)
}

/// Return the name that *cls* gives a hand whose played ranks are *ranks*.
fn name_by_class(cls: HandClass, ranks: &[u8]) -> Result<String, StandingsError> {
    match cls {
        HandClass::Pair => Ok(group_label("Pair", ranks)),
        HandClass::Trips => Ok(group_label("Three of a Kind", ranks)),
        HandClass::Quads => Ok(group_label("Four of a Kind", ranks)),
        HandClass::FiveOfAKind => Ok(group_label("Five of a Kind", ranks)),
        HandClass::TwoPair => Ok(two_pair_label(ranks)),
        HandClass::FullHouse => full_house_label(ranks),
        HandClass::Straight => Ok(straight_label("Straight", ranks)),
        HandClass::StraightFlush => Ok(straight_label("Straight Flush", ranks)),
        HandClass::HighCard => Ok(kicker_label("High Card", "", ranks)),
        HandClass::Flush => Ok(kicker_label("Flush", " high", ranks)),
        HandClass::RoyalFlush => Ok("Royal Flush".to_owned()),
    }
}

/// Return the rank values that *hand* plays: its best five, then each joker's
/// resolution.
fn effective_ranks(hand: &EvaluatedHand) -> Vec<u8> {
    hand.best5()
        .iter()
        .chain(hand.jokers_played_as())
        .filter_map(rank_of_card)
        .collect()
}

/// Return the rank value of a natural card, or `None` for a joker.
fn rank_of_card(card: &Card) -> Option<u8> {
    match card {
        Card::Natural { rank, .. } => Some(rank.value()),
        Card::Joker => None,
    }
}

/// Return a same-rank group class's name from its label and the rank's plural.
fn group_label(label: &str, ranks: &[u8]) -> String {
    format!("{label}{DASH}{}", rank_plural(highest_count_rank(ranks)))
}

/// Return a two-pair name, the higher pair first.
fn two_pair_label(ranks: &[u8]) -> String {
    let pairs = ranks_with_count(ranks, PAIR_COUNT);
    let high = pairs.iter().copied().fold(u8::MIN, u8::max);
    let low = pairs.iter().copied().fold(u8::MAX, u8::min);
    format!("Two Pair{DASH}{} & {}", rank_plural(high), rank_plural(low))
}

/// Return a full house's name, its trips over its pair.
///
/// A multiset without exactly one three-of-a-kind rank and one pair rank is a
/// hands bug; the oracle raises `KeyError` there, so the port returns a typed
/// error rather than guessing.
pub(crate) fn full_house_label(ranks: &[u8]) -> Result<String, StandingsError> {
    let trips = ranks_with_count(ranks, TRIPS_COUNT).first().copied();
    let pair = ranks_with_count(ranks, PAIR_COUNT).first().copied();
    let (Some(trips), Some(pair)) = (trips, pair) else {
        return Err(StandingsError::MalformedHand {
            class: HandClass::FullHouse,
        });
    };
    Ok(format!(
        "Full House{DASH}{} over {}",
        rank_plural(trips),
        rank_plural(pair)
    ))
}

/// Return a straight or straight-flush name, its high card named.
fn straight_label(base: &str, ranks: &[u8]) -> String {
    format!("{base}{DASH}{} high", rank_word(straight_high(ranks)))
}

/// Return the high card that a straight plays; the wheel plays five high.
fn straight_high(ranks: &[u8]) -> u8 {
    let distinct = distinct_ranks(ranks);
    if distinct == WHEEL {
        WHEEL_HIGH
    } else {
        distinct.iter().copied().fold(u8::MIN, u8::max)
    }
}

/// Return a high-card or flush name: the top rank, plus *suffix* when the
/// class adds ` high`.
fn kicker_label(base: &str, suffix: &str, ranks: &[u8]) -> String {
    let high = ranks.iter().copied().fold(u8::MIN, u8::max);
    format!("{base}{DASH}{}{suffix}", rank_word(high))
}

/// Return the rank that *ranks* holds most often; a count tie takes the higher
/// rank.
fn highest_count_rank(ranks: &[u8]) -> u8 {
    distinct_ranks(ranks)
        .iter()
        .copied()
        .fold((0, 0), |best, rank| {
            let candidate = (count_of(ranks, rank), rank);
            if candidate > best { candidate } else { best }
        })
        .1
}

/// Return the distinct ranks that *ranks* holds exactly *count* times,
/// ascending.
fn ranks_with_count(ranks: &[u8], count: usize) -> Vec<u8> {
    distinct_ranks(ranks)
        .into_iter()
        .filter(|rank| count_of(ranks, *rank) == count)
        .collect()
}

/// Return the distinct rank values of *ranks*, ascending.
fn distinct_ranks(ranks: &[u8]) -> Vec<u8> {
    let mut distinct = ranks.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    distinct
}

/// Return how often *rank* appears in *ranks*.
fn count_of(ranks: &[u8], rank: u8) -> usize {
    let mut count = 0;
    for value in ranks {
        if *value == rank {
            count += 1;
        }
    }
    count
}

/// Return the singular word of a rank value.
fn rank_word(value: u8) -> &'static str {
    word(&RANK_WORD, value)
}

/// Return the plural word of a rank value.
fn rank_plural(value: u8) -> &'static str {
    word(&RANK_PLURAL, value)
}

/// Return *value*'s word from *table*, or an empty word when it names no rank.
///
/// Only a [`Card::Natural`] rank reaches here, so a value outside two to ace
/// cannot arrive; the empty word keeps a bad value from panicking.
fn word(table: &'static [&'static str; RANK_COUNT], value: u8) -> &'static str {
    let index = usize::from(value.saturating_sub(LOWEST_RANK));
    table.get(index).copied().unwrap_or_default()
}

#[cfg(test)]
mod tests;
