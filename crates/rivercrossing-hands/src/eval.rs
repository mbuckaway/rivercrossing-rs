// SPDX-License-Identifier: GPL-3.0-only

//! Hand-class detection for a five-card hand.
//!
//! The evaluator counts ranks and suits, so a multi-deck shoe's physically
//! repeated cards are legal input: five naturals of one rank stay four of a kind,
//! and only a joker reaches five of a kind.

use rivercrossing_cards::{Card, Rank, Suit};

use crate::class::HandClass;
use crate::error::HandsError;

/// The number of cards in a full hand.
pub(crate) const HAND_SIZE: usize = 5;
/// The card count as a byte, for the tiebreak prefix.
const HAND_SIZE_U8: u8 = 5;
/// The lowest rank value, two.
pub(crate) const LOWEST_RANK_VALUE: u8 = 2;
/// The highest rank value, ace.
pub(crate) const HIGHEST_RANK_VALUE: u8 = 14;
/// The values that the wheel straight holds; it plays five high.
pub(crate) const WHEEL: [u8; 5] = [2, 3, 4, 5, 14];
/// The high card of the wheel straight.
const WHEEL_HIGH: u8 = 5;
/// The rank span of a straight that is not the wheel.
const STRAIGHT_SPAN: u8 = 4;
/// The rank values of a royal flush.
const ROYAL: [u8; 5] = [10, 11, 12, 13, 14];
/// The ten straights by high card, highest first; five names the wheel.
pub(crate) const STRAIGHT_WINDOWS: [u8; 10] = [14, 13, 12, 11, 10, 9, 8, 7, 6, 5];
/// The suits in declaration order: clubs, diamonds, hearts, spades.
pub(crate) const SUITS: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];
/// The thirteen ranks, lowest first.
pub(crate) const RANKS: [Rank; 13] = [
    Rank::Two,
    Rank::Three,
    Rank::Four,
    Rank::Five,
    Rank::Six,
    Rank::Seven,
    Rank::Eight,
    Rank::Nine,
    Rank::Ten,
    Rank::Jack,
    Rank::Queen,
    Rank::King,
    Rank::Ace,
];

/// One evaluated hand; `(cls, tiebreak)` totally orders hands.
///
/// The tiebreak leads with the hand's card count, so a partial hand ranks below
/// every full hand of its class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedHand {
    cls: HandClass,
    tiebreak: Vec<u8>,
    best5: Vec<Card>,
    jokers_played_as: Vec<Card>,
}

impl EvaluatedHand {
    /// Return a hand from its scored parts.
    #[expect(
        clippy::too_many_arguments,
        reason = "the four arguments are the documented EvaluatedHand fields"
    )]
    pub(crate) fn new(
        cls: HandClass,
        tiebreak: Vec<u8>,
        best5: Vec<Card>,
        jokers_played_as: Vec<Card>,
    ) -> Self {
        Self {
            cls,
            tiebreak,
            best5,
            jokers_played_as,
        }
    }

    /// Return the hand class.
    #[must_use]
    pub const fn cls(&self) -> HandClass {
        self.cls
    }

    /// Return the count-then-kicker tiebreak.
    #[must_use]
    pub fn tiebreak(&self) -> &[u8] {
        &self.tiebreak
    }

    /// Return the five cards that the hand plays.
    #[must_use]
    pub fn best5(&self) -> &[Card] {
        &self.best5
    }

    /// Return each joker's resolution, or nothing for a natural hand.
    #[must_use]
    pub fn jokers_played_as(&self) -> &[Card] {
        &self.jokers_played_as
    }
}

/// Return the rank of a natural card.
pub(crate) fn natural_rank(card: &Card) -> Option<Rank> {
    match card {
        Card::Natural { rank, .. } => Some(*rank),
        Card::Joker => None,
    }
}

/// Return the suit of a natural card.
pub(crate) fn natural_suit(card: &Card) -> Option<Suit> {
    match card {
        Card::Natural { suit, .. } => Some(*suit),
        Card::Joker => None,
    }
}

/// Return the rank value of a natural card.
pub(crate) fn rank_value(card: &Card) -> Option<u8> {
    natural_rank(card).map(Rank::value)
}

/// Return the rank that a value names.
pub(crate) fn rank_from_value(value: u8) -> Option<Rank> {
    RANKS.iter().copied().find(|rank| rank.value() == value)
}

/// Return whether a straight window holds a rank value.
pub(crate) fn window_contains(high: u8, value: u8) -> bool {
    if high == WHEEL_HIGH {
        WHEEL.contains(&value)
    } else {
        value <= high && value.saturating_add(STRAIGHT_SPAN) >= high
    }
}

/// Return the rank values of a straight window, ascending.
pub(crate) fn window_values(high: u8) -> Vec<u8> {
    if high == WHEEL_HIGH {
        return WHEEL.to_vec();
    }
    (high.saturating_sub(STRAIGHT_SPAN)..=high).collect()
}

/// Return the rank values of every natural card in order.
pub(crate) fn card_values(cards: &[Card]) -> Vec<u8> {
    cards.iter().filter_map(rank_value).collect()
}

/// Return the suits of every natural card in order.
fn card_suits(cards: &[Card]) -> Vec<Suit> {
    cards.iter().filter_map(natural_suit).collect()
}

/// Return the class and tiebreak of a natural-card list of up to five cards.
pub(crate) fn evaluate_cards(cards: &[Card]) -> (HandClass, Vec<u8>) {
    let values = card_values(cards);
    let cls = classify_pattern(&values, &card_suits(cards));
    (cls, kicker_tiebreak(cls, &values))
}

/// Return the class of a rank/suit pattern of up to five cards.
///
/// A flush and a straight both need all five cards, so a shorter pattern only
/// ever falls through to its rank-count class.
pub(crate) fn classify_pattern(values: &[u8], suits: &[Suit]) -> HandClass {
    if values.is_empty() {
        return HandClass::HighCard;
    }
    let counts = counts_descending(values);
    if counts.first().is_some_and(|count| *count >= HAND_SIZE_U8) {
        return HandClass::Quads;
    }
    let full = values.len() == HAND_SIZE;
    let is_flush = full && suits.iter().all(|suit| Some(suit) == suits.first());
    let high = if full { straight_high(values) } else { None };
    let by_counts = classify_by_counts(&counts);
    if high.is_some() && is_flush {
        return if is_royal(values) {
            HandClass::RoyalFlush
        } else {
            HandClass::StraightFlush
        };
    }
    if matches!(by_counts, HandClass::Quads | HandClass::FullHouse) {
        return by_counts;
    }
    if is_flush {
        return HandClass::Flush;
    }
    if high.is_some() {
        return HandClass::Straight;
    }
    by_counts
}

/// Return the class that a descending rank-count list names.
fn classify_by_counts(counts: &[u8]) -> HandClass {
    let first = counts.first().copied().unwrap_or(0);
    let second = counts.get(1).copied().unwrap_or(0);
    match (first, second) {
        (4, _) => HandClass::Quads,
        (3, 2) => HandClass::FullHouse,
        (3, _) => HandClass::Trips,
        (2, 2) => HandClass::TwoPair,
        (2, _) => HandClass::Pair,
        _ => HandClass::HighCard,
    }
}

/// Return the rank counts, largest first.
fn counts_descending(values: &[u8]) -> Vec<u8> {
    let frequency = rank_frequency(values);
    let mut counts: Vec<u8> = frequency
        .iter()
        .copied()
        .filter(|count| *count > 0)
        .collect();
    counts.sort_unstable_by(|left, right| right.cmp(left));
    counts
}

/// Return the count of each rank value.
fn rank_frequency(values: &[u8]) -> [u8; 15] {
    let mut frequency: [u8; 15] = [0; 15];
    for &value in values {
        if let Some(slot) = frequency.get_mut(usize::from(value)) {
            *slot = slot.saturating_add(1);
        }
    }
    frequency
}

/// Return the count of one rank value.
fn frequency_of(frequency: &[u8; 15], value: u8) -> u8 {
    frequency.get(usize::from(value)).copied().unwrap_or(0)
}

/// Return the distinct rank values, ascending.
fn distinct_values(values: &[u8]) -> Vec<u8> {
    let mut distinct = values.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    distinct
}

/// Return a straight's high card, or `None` when the values are not one.
///
/// The wheel plays five high, never ace high.
pub(crate) fn straight_high(values: &[u8]) -> Option<u8> {
    let distinct = distinct_values(values);
    if distinct.len() == WHEEL.len() && WHEEL.iter().all(|value| distinct.contains(value)) {
        return Some(WHEEL_HIGH);
    }
    if distinct.len() != HAND_SIZE {
        return None;
    }
    let low = *distinct.first()?;
    let high = *distinct.last()?;
    (high - low == STRAIGHT_SPAN).then_some(high)
}

/// Return whether the rank values are exactly a royal flush's.
fn is_royal(values: &[u8]) -> bool {
    let distinct = distinct_values(values);
    distinct.len() == ROYAL.len() && ROYAL.iter().all(|value| distinct.contains(value))
}

/// Return the count-then-kicker tiebreak for *cls* over *values*.
pub(crate) fn kicker_tiebreak(cls: HandClass, values: &[u8]) -> Vec<u8> {
    let count = card_count(values);
    if values.is_empty() || cls == HandClass::RoyalFlush {
        return vec![count];
    }
    if matches!(cls, HandClass::Straight | HandClass::StraightFlush) {
        return straight_high(values).map_or_else(|| vec![count], |high| vec![count, high]);
    }
    if matches!(cls, HandClass::HighCard | HandClass::Flush) {
        return raw_rank_tiebreak(count, values);
    }
    let frequency = rank_frequency(values);
    let mut ordered: Vec<u8> = (LOWEST_RANK_VALUE..=HIGHEST_RANK_VALUE)
        .filter(|value| frequency_of(&frequency, *value) > 0)
        .collect();
    ordered.sort_unstable_by(|left, right| {
        (frequency_of(&frequency, *right), *right).cmp(&(frequency_of(&frequency, *left), *left))
    });
    let mut tiebreak = vec![count];
    tiebreak.extend(ordered);
    tiebreak
}

/// Return the raw rank-descending tiebreak of a high card or a flush.
fn raw_rank_tiebreak(count: u8, values: &[u8]) -> Vec<u8> {
    let mut tiebreak = vec![count];
    let mut ranks = values.to_vec();
    ranks.sort_unstable_by(|left, right| right.cmp(left));
    tiebreak.extend(ranks);
    tiebreak
}

/// Return the card count as a tiebreak prefix.
fn card_count(values: &[u8]) -> u8 {
    u8::try_from(values.len()).unwrap_or(u8::MAX)
}

/// Return the natural cards of a list, in order.
fn natural_cards(cards: &[Card]) -> Vec<Card> {
    cards
        .iter()
        .filter(|card| !card.is_joker())
        .cloned()
        .collect()
}

/// Evaluate exactly five cards, natural or wild.
///
/// # Errors
///
/// Return [`HandsError::InvalidCardCount`] when `cards` does not hold five cards.
pub fn eval5(cards: &[Card]) -> Result<EvaluatedHand, HandsError> {
    if cards.len() != HAND_SIZE {
        return Err(HandsError::InvalidCardCount { count: cards.len() });
    }
    let naturals = natural_cards(cards);
    if naturals.len() == cards.len() {
        return Ok(evaluate_five_naturals(cards));
    }
    Ok(evaluate_with_wild_cards(cards, &naturals))
}

/// Evaluate five natural cards.
fn evaluate_five_naturals(cards: &[Card]) -> EvaluatedHand {
    let (cls, tiebreak) = evaluate_cards(cards);
    EvaluatedHand::new(cls, tiebreak, cards.to_vec(), Vec::new())
}

/// Resolve every joker to the completion that maximizes the hand.
fn evaluate_with_wild_cards(cards: &[Card], naturals: &[Card]) -> EvaluatedHand {
    let joker_count = cards.len().saturating_sub(naturals.len());
    if let Some(rank) = uniform_natural_rank(naturals) {
        return five_of_a_kind_hand(cards, rank, joker_count);
    }
    let fill = search_best_fill(naturals, joker_count);
    let (cls, tiebreak) = evaluate_cards(&with_fill(naturals, &fill));
    EvaluatedHand::new(cls, tiebreak, cards.to_vec(), fill)
}

/// Evaluate fewer than five cards as the best partial hand they make.
pub(crate) fn partial_hand(cards: &[Card]) -> EvaluatedHand {
    let naturals = natural_cards(cards);
    let joker_count = cards.len().saturating_sub(naturals.len());
    let fill = match uniform_natural_rank(&naturals) {
        Some(rank) => cycle_cards(rank, joker_count),
        None => search_best_fill(&naturals, joker_count),
    };
    let (cls, tiebreak) = evaluate_cards(&with_fill(&naturals, &fill));
    EvaluatedHand::new(cls, tiebreak, cards.to_vec(), fill)
}

/// Return the one rank that every natural card shares, or `None`.
///
/// No natural card means the ace, so an all-joker hand reaches five aces.
pub(crate) fn uniform_natural_rank(naturals: &[Card]) -> Option<Rank> {
    let mut ranks = naturals.iter().filter_map(natural_rank);
    let Some(first) = ranks.next() else {
        return Some(Rank::Ace);
    };
    if ranks.all(|rank| rank == first) {
        Some(first)
    } else {
        None
    }
}

/// Pick *count* suits, cycling the declaration order.
pub(crate) fn cycle_suits(count: usize) -> Vec<Suit> {
    (0..count)
        .filter_map(|index| SUITS.get(index % SUITS.len()).copied())
        .collect()
}

/// Build *count* cards of one rank, cycling the suits.
pub(crate) fn cycle_cards(rank: Rank, count: usize) -> Vec<Card> {
    cycle_suits(count)
        .into_iter()
        .map(|suit| Card::Natural { rank, suit })
        .collect()
}

/// Build the five-of-a-kind result for *rank*.
fn five_of_a_kind_hand(cards: &[Card], rank: Rank, joker_count: usize) -> EvaluatedHand {
    let played = cycle_cards(rank, joker_count);
    EvaluatedHand::new(
        HandClass::FiveOfAKind,
        vec![HAND_SIZE_U8, rank.value()],
        cards.to_vec(),
        played,
    )
}

/// Return the natural cards followed by the joker fill.
pub(crate) fn with_fill(naturals: &[Card], fill: &[Card]) -> Vec<Card> {
    naturals.iter().chain(fill).cloned().collect()
}

/// Return the ranks that could complete or extend a straight.
fn straight_completion_ranks(values: &[u8]) -> Vec<u8> {
    let mut completions = Vec::new();
    if values.iter().all(|value| WHEEL.contains(value)) {
        completions.extend_from_slice(&WHEEL);
    }
    if let (Some(&low), Some(&high)) = (values.iter().min(), values.iter().max())
        && high.saturating_sub(low) <= STRAIGHT_SPAN
    {
        let start = high.saturating_sub(STRAIGHT_SPAN).max(LOWEST_RANK_VALUE);
        let end = low.saturating_add(STRAIGHT_SPAN).min(HIGHEST_RANK_VALUE);
        completions.extend(start..=end);
    }
    completions
}

/// Return the suits that could still reach a flush.
fn plausible_flush_suits(naturals: &[Card], joker_count: usize) -> Vec<Suit> {
    SUITS
        .into_iter()
        .filter(|suit| {
            let count = naturals
                .iter()
                .filter(|card| natural_suit(card) == Some(*suit))
                .count();
            count.saturating_add(joker_count) >= HAND_SIZE
        })
        .collect()
}

/// Return the pruned wild-fill candidates for *naturals*.
fn pruned_wild_candidates(naturals: &[Card], joker_count: usize) -> Vec<Card> {
    let values = card_values(naturals);
    let mut candidate_ranks = values.clone();
    candidate_ranks.extend(straight_completion_ranks(&values));
    candidate_ranks.push(Rank::Ace.value());
    let flush_suits = plausible_flush_suits(naturals, joker_count);
    all_natural_cards()
        .into_iter()
        .filter(|card| {
            natural_rank(card).is_some_and(|rank| candidate_ranks.contains(&rank.value()))
                || natural_suit(card).is_some_and(|suit| flush_suits.contains(&suit))
        })
        .collect()
}

/// Return every natural card, rank-descending then suit order.
fn all_natural_cards() -> Vec<Card> {
    RANKS
        .iter()
        .rev()
        .flat_map(|rank| {
            SUITS
                .into_iter()
                .map(move |suit| Card::Natural { rank: *rank, suit })
        })
        .collect()
}

/// The score of one wild fill: its class, its tiebreak, and whether it repeats a card.
type FillScore = (HandClass, Vec<u8>, bool);

/// One scored fill, with the cards that the fill plays.
type FillOutcome = (FillScore, Vec<Card>);

/// Score one candidate fill against the naturals.
///
/// The last value prefers a duplicate-free fill when class and tiebreak tie.
fn fill_score(naturals: &[Card], fill: &[Card]) -> FillScore {
    let combined = with_fill(naturals, fill);
    let (cls, tiebreak) = evaluate_cards(&combined);
    (cls, tiebreak, is_duplicate_free(&combined))
}

/// Return whether the cards are pairwise distinct.
fn is_duplicate_free(cards: &[Card]) -> bool {
    let mut seen: Vec<&Card> = Vec::new();
    for card in cards {
        if !seen.contains(&card) {
            seen.push(card);
        }
    }
    seen.len() == cards.len()
}

/// The best fill seen so far, with its score.
struct FillSearch<'a> {
    naturals: &'a [Card],
    candidates: &'a [Card],
    picks: Vec<Card>,
    best: Option<FillOutcome>,
}

impl FillSearch<'_> {
    /// Visit every fill that starts at *start* and holds *depth* more cards.
    fn visit(&mut self, start: usize, depth: usize) {
        if depth == 0 {
            self.keep_best_pick();
            return;
        }
        for index in start..self.candidates.len() {
            let next = self.candidates.get(index).cloned();
            if let Some(card) = next {
                self.picks.push(card);
                self.visit(index, depth.saturating_sub(1));
                self.picks.pop();
            }
        }
    }

    /// Record the current picks when they beat the best fill so far.
    fn keep_best_pick(&mut self) {
        let score = fill_score(self.naturals, &self.picks);
        let better = self
            .best
            .as_ref()
            .is_none_or(|(current, _)| current < &score);
        if better {
            self.best = Some((score, self.picks.clone()));
        }
    }
}

/// Search every way to fill the joker slots and keep the best.
fn search_best_fill(naturals: &[Card], joker_count: usize) -> Vec<Card> {
    let candidates = pruned_wild_candidates(naturals, joker_count);
    let mut search = FillSearch {
        naturals,
        candidates: &candidates,
        picks: Vec::new(),
        best: None,
    };
    search.visit(0, joker_count);
    search.best.map(|(_, fill)| fill).unwrap_or_default()
}

#[cfg(test)]
mod tests;
