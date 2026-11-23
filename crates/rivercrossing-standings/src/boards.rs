// SPDX-License-Identifier: GPL-3.0-only

//! The two leaderboards: laps first, then total time.
//!
//! One shared implementation backs both boards (spec §6): a live ride's
//! "fastest time" board is the same key as its laps board, so sorting the time
//! board by time alone would be a bug. Zero-lap and DNS filtering is the
//! caller's job, exactly as the Python oracle's exporters do it.

use std::cmp::Ordering;

use crate::rank::place_runs;
use crate::types::{EntryResult, Placed};

/// Rank the ACTIVE entries by most laps, then shortest total time.
///
/// A `(laps, total_time)` tie is a draw: its entries share a place and carry
/// the draw flag, never a silent order (R-43). Places are numbered over the
/// whole active field before the board is capped at *top*, so a truncated
/// board keeps the places its entries earned; `top` of zero returns no rows.
/// DNF entries never appear. A 0-lap entry appears unless the caller filtered
/// it out first.
#[must_use]
pub fn laps_leaderboard(results: &[EntryResult], top: usize) -> Vec<Placed> {
    leaderboard(results, top)
}

/// Rank the ACTIVE entries by most laps, then shortest total time.
///
/// The board shares [`laps_leaderboard`]'s key and draw rules: the results
/// window's "fastest time" label is display copy, not a different sort.
#[must_use]
pub fn time_leaderboard(results: &[EntryResult], top: usize) -> Vec<Placed> {
    leaderboard(results, top)
}

/// Rank the ACTIVE field by the board key and cap it at *top* rows.
fn leaderboard(results: &[EntryResult], top: usize) -> Vec<Placed> {
    let mut active: Vec<EntryResult> = results
        .iter()
        .filter(|result| !result.dnf)
        .cloned()
        .collect();
    active.sort_by(compare_board_key);
    place_runs(board_runs(&active))
        .into_iter()
        .take(top)
        .collect()
}

/// Order two entries by the board key: most laps first, then shortest time.
///
/// `total_cmp` gives the strict total order that `clippy::float_cmp` requires.
fn compare_board_key(left: &EntryResult, right: &EntryResult) -> Ordering {
    right
        .laps
        .cmp(&left.laps)
        .then_with(|| left.total_time.total_cmp(&right.total_time))
}

/// Return whether two entries hold the same board key.
fn same_board_key(left: &EntryResult, right: &EntryResult) -> bool {
    left.laps == right.laps && left.total_time.total_cmp(&right.total_time).is_eq()
}

/// Merge the key-ordered entries into runs of equal board keys.
fn board_runs(ordered: &[EntryResult]) -> Vec<Vec<EntryResult>> {
    let mut runs: Vec<Vec<EntryResult>> = Vec::new();
    let mut previous: Option<&EntryResult> = None;
    for result in ordered {
        let same = previous.is_some_and(|last| same_board_key(last, result));
        match runs.last_mut() {
            Some(run) if same => run.push(result.clone()),
            _ => runs.push(vec![result.clone()]),
        }
        previous = Some(result);
    }
    runs
}

#[cfg(test)]
mod tests;
