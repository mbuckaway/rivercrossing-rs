// SPDX-License-Identifier: GPL-3.0-only

//! Ranking: hand order, tie-break runs and the team/solo split.

use std::cmp::Ordering;

use rivercrossing_cards::{Card, draw_key};
use rivercrossing_hands::compare::compare;
use rivercrossing_ride::{SnapshotKind, TieBreak};

use crate::DRAW_TIE_NOTE;
use crate::enums::ZeroLapMode;
use crate::error::StandingsError;
use crate::types::{EntryResult, Placed};

/// The composite ascending sort key of one entry within a hand-tie group.
///
/// Each criterion contributes one term: negated laps, total time, or a
/// negated draw key. `f64` holds every term exactly, and `total_cmp` gives the
/// strict total order that `clippy::float_cmp` requires.
type SortKey = Vec<f64>;

/// One auto-rank criterion that contributes a term to a sort key.
#[derive(Debug, Clone, Copy)]
enum Resolver {
    /// Most laps wins.
    Laps,
    /// Shortest total time wins.
    Time,
}

/// Rank *results* best hand first, resolving hand ties by *order*.
///
/// A DNF entry never appears, and every ACTIVE entry is placed, 0-lap ones
/// included. Equal hands are merged into one group and split into runs by the
/// criteria before the venue draw; the draw orders the group only when every
/// member holds the card it drew, and no criterion after it is read.
///
/// # Errors
///
/// Return [`StandingsError::JokerDrawKey`] when a drawn tie reaches a joker
/// draw card.
pub fn rank(results: &[EntryResult], order: &[TieBreak]) -> Result<Vec<Placed>, StandingsError> {
    let mut active: Vec<EntryResult> = results
        .iter()
        .filter(|result| !result.dnf)
        .cloned()
        .collect();
    active.sort_by(|left, right| compare(&right.hand, &left.hand));
    let mut runs: Vec<Vec<EntryResult>> = Vec::new();
    for group in hand_groups(active) {
        runs.extend(resolved_runs(&group, order)?);
    }
    Ok(place_runs(runs))
}

/// Rank a mixed field as two sections: teams, then solos.
///
/// Each section ranks independently and numbers its places from one. The
/// zero-lap mode applies per section, so neither kind's DNS tail appears in the
/// other's list.
///
/// # Errors
///
/// Return [`StandingsError::JokerDrawKey`] when a drawn tie in either section
/// reaches a joker draw card.
pub fn rank_by_kind(
    results: &[EntryResult],
    order: &[TieBreak],
    zero_laps: ZeroLapMode,
) -> Result<(Vec<Placed>, Vec<Placed>), StandingsError> {
    let teams = section(results, SnapshotKind::Team);
    let solo = section(results, SnapshotKind::Solo);
    Ok((
        rank_section(&teams, order, zero_laps)?,
        rank_section(&solo, order, zero_laps)?,
    ))
}

/// Return the entries of one kind, in input order.
fn section(results: &[EntryResult], kind: SnapshotKind) -> Vec<EntryResult> {
    results
        .iter()
        .filter(|result| result.kind == kind)
        .cloned()
        .collect()
}

/// Rank one kind's entries under `order` and the `zero_laps` mode.
fn rank_section(
    results: &[EntryResult],
    order: &[TieBreak],
    zero_laps: ZeroLapMode,
) -> Result<Vec<Placed>, StandingsError> {
    match zero_laps {
        ZeroLapMode::Rank => rank(results, order),
        ZeroLapMode::Hide => rank(&without_dns_candidates(results), order),
        ZeroLapMode::Dns => {
            let mut placed = rank(&without_dns_candidates(results), order)?;
            placed.extend(
                results
                    .iter()
                    .filter(|result| is_dns_candidate(result))
                    .map(dns_placed),
            );
            Ok(placed)
        }
    }
}

/// Return whether *result* is a 0-lap ACTIVE entry: a DNS row.
fn is_dns_candidate(result: &EntryResult) -> bool {
    !result.dnf && result.laps == 0
}

/// Return the entries that are not DNS candidates.
fn without_dns_candidates(results: &[EntryResult]) -> Vec<EntryResult> {
    results
        .iter()
        .filter(|result| !is_dns_candidate(result))
        .cloned()
        .collect()
}

/// Build the unplaced DNS row for one 0-lap ACTIVE entry.
fn dns_placed(result: &EntryResult) -> Placed {
    Placed {
        place: None,
        result: result.clone(),
        tie_note: None,
        draw_required: false,
    }
}

/// Merge a hand-sorted field into consecutive equal-hand groups.
fn hand_groups(active: Vec<EntryResult>) -> Vec<Vec<EntryResult>> {
    let mut groups: Vec<Vec<EntryResult>> = Vec::new();
    for result in active {
        match groups.last_mut() {
            Some(group) if holds_same_hand(group, &result) => group.push(result),
            _ => groups.push(vec![result]),
        }
    }
    groups
}

/// Return whether *group*'s last entry holds the same hand as *result*.
fn holds_same_hand(group: &[EntryResult], result: &EntryResult) -> bool {
    group
        .last()
        .is_some_and(|last| compare(&last.hand, &result.hand) == Ordering::Equal)
}

/// Split one hand-tie *group* into place runs under *order*.
fn resolved_runs(
    group: &[EntryResult],
    order: &[TieBreak],
) -> Result<Vec<Vec<EntryResult>>, StandingsError> {
    let criteria = resolvers(order);
    let drawn = order.contains(&TieBreak::HighCard)
        && group.iter().all(|result| result.tiebreak_card.is_some());
    let mut keyed: Vec<(SortKey, EntryResult)> = group
        .iter()
        .map(|result| (base_key(result, &criteria), result.clone()))
        .collect();
    if drawn {
        // `drawn` guarantees one card per entry, so this zip pairs every key
        // with the card of the entry it came from.
        let cards = group
            .iter()
            .filter_map(|result| result.tiebreak_card.as_ref());
        for ((key, _), card) in keyed.iter_mut().zip(cards) {
            key.push(negated_draw_key(card)?);
        }
    }
    keyed.sort_by(|(left, _), (right, _)| compare_keys(left, right));
    Ok(merge_runs(keyed))
}

/// Read the auto-rank criteria before the venue draw.
///
/// The walk stops at [`TieBreak::HighCard`], so a criterion after it is never
/// read.
fn resolvers(order: &[TieBreak]) -> Vec<Resolver> {
    order
        .iter()
        .map_while(|criterion| match criterion {
            TieBreak::Laps => Some(Resolver::Laps),
            TieBreak::TotalTime => Some(Resolver::Time),
            TieBreak::HighCard => None,
        })
        .collect()
}

/// Build the sort key of the criteria before the draw.
fn base_key(result: &EntryResult, criteria: &[Resolver]) -> SortKey {
    criteria
        .iter()
        .map(|resolver| match resolver {
            Resolver::Laps => -f64::from(result.laps),
            Resolver::Time => result.total_time,
        })
        .collect()
}

/// Return the negated draw key of *card*, so the higher card sorts first.
fn negated_draw_key(card: &Card) -> Result<f64, StandingsError> {
    let key = draw_key(card).map_err(|_| StandingsError::JokerDrawKey)?;
    Ok(-f64::from(key))
}

/// Compare two composite sort keys ascending.
///
/// Two keys of one group always hold one term per criterion, so a length
/// difference never decides the order.
fn compare_keys(left: &[f64], right: &[f64]) -> Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| left.total_cmp(right))
        .find(|ordering| *ordering != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

/// Merge stable-sorted keyed entries into runs of equal keys.
fn merge_runs(keyed: Vec<(SortKey, EntryResult)>) -> Vec<Vec<EntryResult>> {
    let mut runs: Vec<(SortKey, Vec<EntryResult>)> = Vec::new();
    for (key, result) in keyed {
        match runs.last_mut() {
            Some((last_key, run)) if compare_keys(last_key, &key) == Ordering::Equal => {
                run.push(result);
            }
            _ => runs.push((key, vec![result])),
        }
    }
    runs.into_iter().map(|(_, run)| run).collect()
}

/// Assign competition places to *runs*; a run's entries share its place.
///
/// The leaderboards share this walk, so both surfaces number places and mark
/// a draw the same way.
pub(crate) fn place_runs(runs: Vec<Vec<EntryResult>>) -> Vec<Placed> {
    let mut placed: Vec<Placed> = Vec::new();
    let mut place: u32 = 1;
    for run in runs {
        let is_draw = run.len() > 1;
        let tie_note = is_draw.then(|| DRAW_TIE_NOTE.to_owned());
        let width = u32::try_from(run.len()).unwrap_or(u32::MAX);
        placed.extend(run.into_iter().map(|result| Placed {
            place: Some(place),
            result,
            tie_note: tie_note.clone(),
            draw_required: is_draw,
        }));
        place = place.saturating_add(width);
    }
    placed
}

#[cfg(test)]
mod tests;
