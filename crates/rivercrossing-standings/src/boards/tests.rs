// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the leaderboards.
//!
//! Both boards share one key, so every ordering case runs against each of
//! them: sorting the "time" board by time alone is the bug these pin.

use pretty_assertions::assert_eq;
use rivercrossing_hands::best_hand;
use rivercrossing_ride::{EntryId, SnapshotKind};
use rstest::rstest;

use super::{laps_leaderboard, time_leaderboard};
use crate::types::{EntryResult, Placed};
use crate::{DEFAULT_TOP, DRAW_TIE_NOTE};

/// Build one ACTIVE solo entry with *laps* recorded laps and `total_time`
/// seconds.
fn result(id: &str, laps: u32, total_time: f64) -> EntryResult {
    EntryResult {
        entry_id: EntryId(id.to_owned()),
        plate: id.to_owned(),
        name: "Rider".to_owned(),
        kind: SnapshotKind::Solo,
        laps,
        total_time,
        best_lap: 0.0,
        cards: Vec::new(),
        hand: best_hand(&[]),
        dnf: false,
        sex: None,
        tiebreak_card: None,
    }
}

/// Return *result* marked DNF.
fn as_dnf(result: EntryResult) -> EntryResult {
    EntryResult {
        dnf: true,
        ..result
    }
}

/// Build a twelve-entry field whose laps rise with the id, 0 to 11.
fn twelve_lap_field() -> Vec<EntryResult> {
    (0..12u32)
        .map(|laps| result(&laps.to_string(), laps, 0.0))
        .collect()
}

/// Return the entry ids of *placed*, in order.
fn ids(placed: &[Placed]) -> Vec<String> {
    placed
        .iter()
        .map(|row| row.result.entry_id.0.clone())
        .collect()
}

/// Return the places of *placed*, in order.
fn places(placed: &[Placed]) -> Vec<Option<u32>> {
    placed.iter().map(|row| row.place).collect()
}

/// Return the draw flags of *placed*, in order.
fn flags(placed: &[Placed]) -> Vec<bool> {
    placed.iter().map(|row| row.draw_required).collect()
}

/// Return the tie notes of *placed*, in order.
fn notes(placed: &[Placed]) -> Vec<Option<String>> {
    placed.iter().map(|row| row.tie_note.clone()).collect()
}

// ---------------------------------------------------------------- ordering

#[test]
fn test_laps_leaderboard_orders_by_most_laps_then_shortest_time() {
    let field = vec![
        result("1", 5, 100.0),
        result("2", 3, 90.0),
        result("3", 3, 80.0),
        result("4", 2, 50.0),
    ];

    let placed = laps_leaderboard(&field, 2);

    assert_eq!(ids(&placed), vec!["1", "3"]);
    assert_eq!(places(&placed), vec![Some(1), Some(2)]);
}

#[test]
fn test_time_leaderboard_orders_by_most_laps_then_shortest_time() {
    let many_laps_slow = result("1", 3, 300.0);
    let few_laps_fast = result("2", 2, 50.0);

    let placed = time_leaderboard(&[many_laps_slow, few_laps_fast], 1);

    assert_eq!(ids(&placed), vec!["1"]);
}

#[test]
fn test_laps_leaderboard_orders_by_laps_then_time_not_time_alone() {
    let three_laps_fast = result("1", 3, 1200.0);
    let five_laps_slow = result("2", 5, 3600.0);
    let five_laps_slower = result("3", 5, 4200.0);

    let placed = laps_leaderboard(
        &[three_laps_fast, five_laps_slow, five_laps_slower],
        DEFAULT_TOP,
    );

    assert_eq!(ids(&placed), vec!["2", "3", "1"]);
}

#[test]
fn test_time_leaderboard_orders_by_laps_then_time_not_time_alone() {
    let three_laps_fast = result("1", 3, 1200.0);
    let five_laps_slow = result("2", 5, 3600.0);
    let five_laps_slower = result("3", 5, 4200.0);

    let placed = time_leaderboard(
        &[three_laps_fast, five_laps_slow, five_laps_slower],
        DEFAULT_TOP,
    );

    assert_eq!(ids(&placed), vec!["2", "3", "1"]);
}

#[test]
fn test_laps_leaderboard_equal_keys_keep_input_order() {
    let first_input = result("2", 3, 100.0);
    let second_input = result("1", 3, 100.0);

    let placed = laps_leaderboard(&[first_input, second_input], DEFAULT_TOP);

    assert_eq!(ids(&placed), vec!["2", "1"]);
}

// ------------------------------------------------------------ top boundary

#[rstest]
#[case(0, 0)]
#[case(9, 9)]
#[case(10, 10)]
#[case(11, 11)]
fn test_laps_leaderboard_top_boundary_returns_capped_length(
    #[case] top: usize,
    #[case] expected: usize,
) {
    let placed = laps_leaderboard(&twelve_lap_field(), top);

    assert_eq!(placed.len(), expected);
}

#[test]
fn test_laps_leaderboard_top_one_returns_the_leader() {
    let placed = laps_leaderboard(&twelve_lap_field(), 1);

    assert_eq!(ids(&placed), vec!["11"]);
    assert_eq!(places(&placed), vec![Some(1)]);
}

#[test]
fn test_laps_leaderboard_top_past_the_field_returns_every_row() {
    let field = vec![
        result("1", 5, 100.0),
        result("2", 4, 90.0),
        result("3", 3, 80.0),
    ];

    let placed = laps_leaderboard(&field, 20);

    assert_eq!(places(&placed), vec![Some(1), Some(2), Some(3)]);
}

// ------------------------------------------------------------------- draws

#[test]
fn test_laps_leaderboard_equal_laps_and_time_flags_a_draw() {
    let first = result("1", 5, 100.0);
    let second = result("2", 5, 100.0);

    let placed = laps_leaderboard(&[first, second], DEFAULT_TOP);

    assert_eq!(places(&placed), vec![Some(1), Some(1)]);
    assert_eq!(flags(&placed), vec![true, true]);
    assert_eq!(notes(&placed), vec![Some(DRAW_TIE_NOTE.to_owned()); 2]);
}

#[test]
fn test_time_leaderboard_equal_laps_and_time_flags_a_draw() {
    let first = result("1", 5, 100.0);
    let second = result("2", 5, 100.0);

    let placed = time_leaderboard(&[first, second], DEFAULT_TOP);

    assert_eq!(places(&placed), vec![Some(1), Some(1)]);
    assert_eq!(flags(&placed), vec![true, true]);
    assert_eq!(notes(&placed), vec![Some(DRAW_TIE_NOTE.to_owned()); 2]);
}

#[test]
fn test_laps_leaderboard_three_way_tie_takes_places_one_to_three() {
    let field = vec![
        result("1", 5, 100.0),
        result("2", 5, 100.0),
        result("3", 5, 100.0),
        result("4", 4, 50.0),
    ];

    let placed = laps_leaderboard(&field, DEFAULT_TOP);

    assert_eq!(places(&placed), vec![Some(1), Some(1), Some(1), Some(4)]);
    assert_eq!(flags(&placed), vec![true, true, true, false]);
}

#[test]
fn test_laps_leaderboard_tie_straddling_top_keeps_full_field_places() {
    let field = vec![
        result("1", 5, 100.0),
        result("2", 5, 100.0),
        result("3", 4, 50.0),
    ];

    let placed = laps_leaderboard(&field, 2);

    assert_eq!(ids(&placed), vec!["1", "2"]);
    assert_eq!(places(&placed), vec![Some(1), Some(1)]);
}

#[test]
fn test_laps_leaderboard_top_inside_a_draw_run_keeps_its_shared_place() {
    let field = vec![
        result("1", 5, 100.0),
        result("2", 5, 100.0),
        result("3", 4, 50.0),
    ];

    let placed = laps_leaderboard(&field, 1);

    assert_eq!(ids(&placed), vec!["1"]);
    assert_eq!(places(&placed), vec![Some(1)]);
}

#[test]
fn test_laps_leaderboard_truncation_does_not_renumber_places() {
    let field = vec![
        result("1", 5, 100.0),
        result("2", 5, 100.0),
        result("3", 4, 50.0),
        result("4", 3, 40.0),
    ];

    let placed = laps_leaderboard(&field, 3);

    assert_eq!(ids(&placed), vec!["1", "2", "3"]);
    assert_eq!(places(&placed), vec![Some(1), Some(1), Some(3)]);
}

// ------------------------------------------------------------ DNF and DNS

#[test]
fn test_laps_leaderboard_excludes_a_dnf_entry() {
    let dnf = as_dnf(result("1", 99, 10.0));
    let active = result("2", 4, 90.0);

    let placed = laps_leaderboard(&[dnf, active], DEFAULT_TOP);

    assert_eq!(ids(&placed), vec!["2"]);
    assert_eq!(places(&placed), vec![Some(1)]);
}

#[test]
fn test_time_leaderboard_excludes_a_dnf_entry() {
    let dnf = as_dnf(result("1", 99, 10.0));
    let active = result("2", 4, 90.0);

    let placed = time_leaderboard(&[dnf, active], DEFAULT_TOP);

    assert_eq!(ids(&placed), vec!["2"]);
    assert_eq!(places(&placed), vec![Some(1)]);
}

#[test]
fn test_laps_leaderboard_includes_a_zero_lap_entry() {
    let zero_laps = result("1", 0, 0.0);
    let one_lap = result("2", 1, 600.0);

    let placed = laps_leaderboard(&[zero_laps, one_lap], DEFAULT_TOP);

    assert_eq!(ids(&placed), vec!["2", "1"]);
    assert_eq!(places(&placed), vec![Some(1), Some(2)]);
}

// ------------------------------------------------------------ empty field

#[test]
fn test_laps_leaderboard_empty_field_returns_empty() {
    let placed = laps_leaderboard(&[], DEFAULT_TOP);

    assert_eq!(placed.len(), 0);
}

#[test]
fn test_time_leaderboard_empty_field_returns_empty() {
    let placed = time_leaderboard(&[], DEFAULT_TOP);

    assert_eq!(placed.len(), 0);
}
