// SPDX-License-Identifier: GPL-3.0-only

//! The best five-card hand from a larger set of cards.
//!
//! The search builds one candidate five-card hand per reachable shape from rank
//! and suit counts, never by enumerating natural subsets, so it stays linear in
//! the pool size. A joker plays only where it improves the hand, so a surplus
//! joker stays unused and an equal natural hand beats a wild one.

use std::cmp::Reverse;

use rivercrossing_cards::{Card, Rank, Suit};

use crate::class::HandClass;
use crate::eval::{
    EvaluatedHand, HAND_SIZE, RANKS, STRAIGHT_WINDOWS, SUITS, cycle_cards, cycle_suits,
    evaluate_cards, natural_rank, natural_suit, partial_hand, rank_from_value, rank_value,
    window_contains, window_values, with_fill,
};

/// The group shapes of a rank-count candidate, largest group first.
const SHAPES: [&[usize]; 7] = [&[5], &[4], &[3, 2], &[3], &[2, 2], &[2], &[]];

/// One rank group of a candidate hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RankAssignment {
    rank: Rank,
    naturals_used: usize,
    jokers_used: usize,
}

/// Find the best five-card hand within `cards`.
///
/// Fewer than five cards score as the best partial hand that they make. Five or
/// more cards try every joker count that can assemble five cards and keep the
/// strongest `(class, tiebreak)` key, preferring fewer jokers on a tie.
#[must_use]
pub fn best_hand(cards: &[Card]) -> EvaluatedHand {
    if cards.len() < HAND_SIZE {
        return partial_hand(cards);
    }
    let naturals: Vec<Card> = cards
        .iter()
        .filter(|card| !card.is_joker())
        .cloned()
        .collect();
    let jokers: Vec<Card> = cards
        .iter()
        .filter(|card| card.is_joker())
        .cloned()
        .collect();
    let lowest = HAND_SIZE.saturating_sub(naturals.len());
    let highest = jokers.len().min(HAND_SIZE);
    let mut best: Option<(usize, EvaluatedHand)> = None;
    for joker_count in lowest..=highest {
        let Some(active) = jokers.get(..joker_count) else {
            continue;
        };
        for hand in candidate_hands(&naturals, active) {
            if is_better(best.as_ref(), &hand, joker_count) {
                best = Some((joker_count, hand));
            }
        }
    }
    best.map_or_else(|| partial_hand(cards), |(_, hand)| hand)
}

/// Return whether *hand* beats the hand that is best so far.
fn is_better(
    current: Option<&(usize, EvaluatedHand)>,
    hand: &EvaluatedHand,
    joker_count: usize,
) -> bool {
    match current {
        None => true,
        Some((current_count, current_hand)) => {
            rank_key(hand, joker_count) > rank_key(current_hand, *current_count)
        }
    }
}

/// Return the comparison key of a candidate hand.
fn rank_key(hand: &EvaluatedHand, joker_count: usize) -> (HandClass, &[u8], Reverse<usize>) {
    (hand.cls(), hand.tiebreak(), Reverse(joker_count))
}

/// Return one candidate hand per reachable shape, in the oracle's order.
fn candidate_hands(naturals: &[Card], jokers: &[Card]) -> Vec<EvaluatedHand> {
    let straight_flush = straight_flush_candidate(naturals, jokers);
    let flush = flush_candidate(naturals, jokers);
    let straight = straight_candidate(naturals, jokers);
    straight_flush
        .into_iter()
        .chain(flush)
        .chain(straight)
        .chain(
            SHAPES
                .iter()
                .filter_map(|shape| grouped_candidate(naturals, jokers, shape)),
        )
        .collect()
}

/// Return the best straight flush reachable in any one suit.
fn straight_flush_candidate(naturals: &[Card], jokers: &[Card]) -> Option<EvaluatedHand> {
    let need = HAND_SIZE.saturating_sub(jokers.len());
    let mut best: Option<(u8, Suit)> = None;
    for suit in SUITS {
        let present = suited_values(naturals, suit);
        for high in STRAIGHT_WINDOWS {
            let in_window = present
                .iter()
                .filter(|value| window_contains(high, **value))
                .count();
            if in_window < need {
                continue;
            }
            if best.is_none_or(|(best_high, _)| high > best_high) {
                best = Some((high, suit));
            }
            break;
        }
    }
    let (high, suit) = best?;
    let present = suited_values(naturals, suit);
    let window = window_values(high);
    let mut covered: Vec<u8> = window
        .iter()
        .copied()
        .filter(|value| present.contains(value))
        .collect();
    covered.sort_unstable_by(|left, right| right.cmp(left));
    covered.truncate(need);
    let chosen: Vec<Card> = covered
        .iter()
        .filter_map(|value| last_card_in_suit(naturals, suit, *value).cloned())
        .collect();
    let missing = missing_values(&window, &covered);
    let played: Vec<Card> = missing
        .iter()
        .filter_map(|value| rank_from_value(*value).map(|rank| Card::Natural { rank, suit }))
        .collect();
    Some(evaluate_candidate(&chosen, &played, jokers))
}

/// Return the best flush reachable in any one suit.
fn flush_candidate(naturals: &[Card], jokers: &[Card]) -> Option<EvaluatedHand> {
    let need = HAND_SIZE.saturating_sub(jokers.len());
    let mut best: Option<(Suit, Vec<Card>)> = None;
    for suit in SUITS {
        let mut suited: Vec<Card> = naturals
            .iter()
            .filter(|card| natural_suit(card) == Some(suit))
            .cloned()
            .collect();
        suited.sort_by_key(|card| Reverse(rank_value(card).unwrap_or_default()));
        if suited.len() < need {
            continue;
        }
        let chosen: Vec<Card> = suited.into_iter().take(need).collect();
        let better = best
            .as_ref()
            .is_none_or(|(_, current)| ranks_of(&chosen) > ranks_of(current));
        if better {
            best = Some((suit, chosen));
        }
    }
    let (suit, chosen) = best?;
    let played: Vec<Card> = jokers
        .iter()
        .map(|_| Card::Natural {
            rank: Rank::Ace,
            suit,
        })
        .collect();
    Some(evaluate_candidate(&chosen, &played, jokers))
}

/// Return the best plain straight reachable over the global ranks.
fn straight_candidate(naturals: &[Card], jokers: &[Card]) -> Option<EvaluatedHand> {
    let need = HAND_SIZE.saturating_sub(jokers.len());
    for high in STRAIGHT_WINDOWS {
        let window = window_values(high);
        let present: Vec<u8> = window
            .iter()
            .copied()
            .filter(|value| has_rank(naturals, *value))
            .collect();
        if present.len() < need {
            continue;
        }
        let mut chosen_values = present;
        chosen_values.sort_unstable_by(|left, right| right.cmp(left));
        chosen_values.truncate(need);
        let chosen: Vec<Card> = chosen_values
            .iter()
            .filter_map(|value| last_card_of_rank(naturals, *value).cloned())
            .collect();
        let missing = missing_values(&window, &chosen_values);
        let played: Vec<Card> = missing
            .iter()
            .zip(cycle_suits(missing.len()))
            .filter_map(|(value, suit)| {
                rank_from_value(*value).map(|rank| Card::Natural { rank, suit })
            })
            .collect();
        return Some(evaluate_candidate(&chosen, &played, jokers));
    }
    None
}

/// Return the best five-card hand shaped like `group_sizes`, if any.
fn grouped_candidate(
    naturals: &[Card],
    jokers: &[Card],
    group_sizes: &[usize],
) -> Option<EvaluatedHand> {
    let counts = count_by_rank(naturals);
    let slots = rank_slots(group_sizes);
    let assignment = greedy_rank_assignment(&counts, &slots, jokers.len())?;
    let spent: usize = assignment.iter().map(|entry| entry.jokers_used).sum();
    let assignment = spend_leftover_jokers(&assignment, jokers.len().saturating_sub(spent));
    let groups = group_naturals(naturals);
    let mut chosen: Vec<Card> = Vec::new();
    let mut played: Vec<Card> = Vec::new();
    for entry in &assignment {
        let available = cards_of(&groups, entry.rank);
        let take = available.get(..entry.naturals_used).unwrap_or(available);
        chosen.extend_from_slice(take);
        played.extend(cycle_cards(entry.rank, entry.jokers_used));
    }
    Some(evaluate_candidate(&chosen, &played, jokers))
}

/// Return `group_sizes` padded with single-card kicker slots to five.
fn rank_slots(group_sizes: &[usize]) -> Vec<usize> {
    let used: usize = group_sizes.iter().sum();
    let kickers = HAND_SIZE.saturating_sub(used);
    group_sizes
        .iter()
        .copied()
        .chain(std::iter::repeat_n(1, kickers))
        .collect()
}

/// Return one assignment per slot, or `None` when the jokers cannot fill them.
fn greedy_rank_assignment(
    counts: &[usize; 15],
    slots: &[usize],
    joker_count: usize,
) -> Option<Vec<RankAssignment>> {
    let mut used: Vec<Rank> = Vec::new();
    let mut jokers_left = joker_count;
    let mut assignment = Vec::with_capacity(slots.len());
    for &size in slots {
        let found = RANKS
            .iter()
            .rev()
            .filter(|rank| !used.contains(rank))
            .find_map(|rank| {
                let have = count_of(counts, *rank);
                let cost = size.saturating_sub(have);
                (cost <= jokers_left).then_some((*rank, have.min(size), cost))
            });
        let (rank, naturals_used, jokers_used) = found?;
        assignment.push(RankAssignment {
            rank,
            naturals_used,
            jokers_used,
        });
        used.push(rank);
        jokers_left = jokers_left.saturating_sub(jokers_used);
    }
    Some(assignment)
}

/// Re-label natural cards as jokers of the same rank until none are left.
fn spend_leftover_jokers(assignment: &[RankAssignment], leftover: usize) -> Vec<RankAssignment> {
    let mut remaining = leftover;
    let mut spent = Vec::with_capacity(assignment.len());
    for entry in assignment {
        let take = remaining.min(entry.naturals_used);
        spent.push(RankAssignment {
            rank: entry.rank,
            naturals_used: entry.naturals_used.saturating_sub(take),
            jokers_used: entry.jokers_used.saturating_add(take),
        });
        remaining = remaining.saturating_sub(take);
    }
    spent
}

/// Return the count of each rank among the naturals.
fn count_by_rank(naturals: &[Card]) -> [usize; 15] {
    let mut counts: [usize; 15] = [0; 15];
    for card in naturals {
        if let Some(rank) = natural_rank(card)
            && let Some(slot) = counts.get_mut(usize::from(rank.value()))
        {
            *slot = slot.saturating_add(1);
        }
    }
    counts
}

/// Return the count of one rank.
fn count_of(counts: &[usize; 15], rank: Rank) -> usize {
    counts.get(usize::from(rank.value())).copied().unwrap_or(0)
}

/// Return each natural card grouped by rank, in first-seen order.
fn group_naturals(naturals: &[Card]) -> Vec<(Rank, Vec<Card>)> {
    let mut groups: Vec<(Rank, Vec<Card>)> = Vec::new();
    for card in naturals {
        let Some(rank) = natural_rank(card) else {
            continue;
        };
        match groups.iter_mut().find(|(grouped, _)| *grouped == rank) {
            Some((_, list)) => list.push(card.clone()),
            None => groups.push((rank, vec![card.clone()])),
        }
    }
    groups
}

/// Return the naturals of one rank group.
fn cards_of(groups: &[(Rank, Vec<Card>)], rank: Rank) -> &[Card] {
    groups
        .iter()
        .find(|(grouped, _)| *grouped == rank)
        .map_or(&[], |(_, cards)| cards.as_slice())
}

/// Return the distinct rank values of one suit, in first-seen order.
fn suited_values(naturals: &[Card], suit: Suit) -> Vec<u8> {
    let mut values: Vec<u8> = Vec::new();
    for card in naturals {
        if natural_suit(card) == Some(suit)
            && let Some(value) = rank_value(card)
            && !values.contains(&value)
        {
            values.push(value);
        }
    }
    values
}

/// Return the last natural card of one rank and suit.
fn last_card_in_suit(naturals: &[Card], suit: Suit, value: u8) -> Option<&Card> {
    naturals
        .iter()
        .rev()
        .find(|card| natural_suit(card) == Some(suit) && rank_value(card) == Some(value))
}

/// Return the last natural card of one rank.
fn last_card_of_rank(naturals: &[Card], value: u8) -> Option<&Card> {
    naturals
        .iter()
        .rev()
        .find(|card| rank_value(card) == Some(value))
}

/// Return whether any natural card holds a rank value.
fn has_rank(naturals: &[Card], value: u8) -> bool {
    naturals.iter().any(|card| rank_value(card) == Some(value))
}

/// Return the window values that are not covered, highest first.
fn missing_values(window: &[u8], covered: &[u8]) -> Vec<u8> {
    let mut missing: Vec<u8> = window
        .iter()
        .copied()
        .filter(|value| !covered.contains(value))
        .collect();
    missing.sort_unstable_by(|left, right| right.cmp(left));
    missing
}

/// Return the rank values of the cards, in order.
fn ranks_of(cards: &[Card]) -> Vec<u8> {
    cards.iter().filter_map(rank_value).collect()
}

/// Score one candidate's chosen cards and wrap it for `best_hand`.
fn evaluate_candidate(chosen: &[Card], played: &[Card], jokers: &[Card]) -> EvaluatedHand {
    let combined = with_fill(chosen, played);
    let (mut cls, tiebreak) = evaluate_cards(&combined);
    if !played.is_empty() && all_one_rank(&combined) {
        cls = HandClass::FiveOfAKind;
    }
    let best5: Vec<Card> = chosen.iter().chain(jokers).cloned().collect();
    EvaluatedHand::new(cls, tiebreak, best5, played.to_vec())
}

/// Return whether every card holds one rank.
fn all_one_rank(cards: &[Card]) -> bool {
    let mut ranks: Vec<Rank> = Vec::new();
    for rank in cards.iter().filter_map(natural_rank) {
        if !ranks.contains(&rank) {
            ranks.push(rank);
        }
    }
    ranks.len() == 1
}

#[cfg(test)]
mod tests;
