// SPDX-License-Identifier: GPL-3.0-only

use std::cmp::Ordering;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{Duration, Instant};

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;

use super::{build_report, compare_total_order, seeded_compare_sample, seeded_field, self_test};
use crate::best::best_hand;
use crate::class::HandClass;
use crate::compare::compare;
use crate::eval::EvaluatedHand;
use crate::vectors::{JokerVectorRow, RankSweepRow, joker_vector_rows, rank_sweep_rows};

/// The fake clock's call counter, read only by the slow-field test.
static SLOW_CLOCK_TICKS: AtomicU64 = AtomicU64::new(0);

/// Return a clock that advances two seconds on its second call.
fn slow_clock() -> Instant {
    let ticks = SLOW_CLOCK_TICKS.fetch_add(1, AtomicOrdering::Relaxed);
    Instant::now() + Duration::from_secs(ticks.saturating_mul(2))
}

fn committed_rank_rows() -> Vec<RankSweepRow> {
    rank_sweep_rows().expect("the committed sweep parses")
}

fn committed_joker_rows() -> Vec<JokerVectorRow> {
    joker_vector_rows().expect("the committed joker table parses")
}

fn six_joker_hand(_cards: &[Card]) -> EvaluatedHand {
    EvaluatedHand::new(
        HandClass::FiveOfAKind,
        vec![5, 14],
        Vec::new(),
        vec![Card::Joker; 6],
    )
}

fn always_greater(_left: &EvaluatedHand, _right: &EvaluatedHand) -> Ordering {
    Ordering::Greater
}

fn always_less(_left: &EvaluatedHand, _right: &EvaluatedHand) -> Ordering {
    Ordering::Less
}

fn rank_rows_with_a_wrong_rank() -> Vec<RankSweepRow> {
    let rows = committed_rank_rows();
    let first = rows.first().expect("the sweep holds a first row");
    let mut corrupted = rows.clone();
    if let Some(slot) = corrupted.first_mut() {
        *slot = RankSweepRow::from_parts(first.cards().to_vec(), 2, first.class());
    }
    corrupted
}

fn rank_rows_with_a_short_row() -> Vec<RankSweepRow> {
    let rows = committed_rank_rows();
    let first = rows.first().expect("the sweep holds a first row");
    let cards: Vec<Card> = first.cards().iter().take(4).cloned().collect();
    let mut corrupted = rows.clone();
    if let Some(slot) = corrupted.first_mut() {
        *slot = RankSweepRow::from_parts(cards, first.rank(), first.class());
    }
    corrupted
}

fn rank_rows_with_duplicate_keys() -> Vec<RankSweepRow> {
    let rows = committed_rank_rows();
    let first = rows.first().expect("the sweep holds a first row");
    let second = rows.get(1).expect("the sweep holds a second row");
    let mut corrupted = rows.clone();
    if let Some(slot) = corrupted.first_mut() {
        *slot = RankSweepRow::from_parts(second.cards().to_vec(), first.rank(), second.class());
    }
    corrupted
}

fn joker_rows_with_a_wrong_class() -> Vec<JokerVectorRow> {
    let rows = committed_joker_rows();
    let first = rows.first().expect("the joker table holds a first row");
    let mut corrupted = rows.clone();
    if let Some(slot) = corrupted.first_mut() {
        *slot = JokerVectorRow::from_parts(
            first.cards().to_vec(),
            HandClass::RoyalFlush,
            first.expected_tiebreak().to_vec(),
        );
    }
    corrupted
}

fn joker_rows_with_a_wrong_tiebreak() -> Vec<JokerVectorRow> {
    let rows = committed_joker_rows();
    let first = rows.first().expect("the joker table holds a first row");
    let mut corrupted = rows.clone();
    if let Some(slot) = corrupted.first_mut() {
        *slot = JokerVectorRow::from_parts(first.cards().to_vec(), first.expected_class(), vec![0]);
    }
    corrupted
}

fn joker_rows_with_a_short_row() -> Vec<JokerVectorRow> {
    let rows = committed_joker_rows();
    let first = rows.first().expect("the joker table holds a first row");
    let cards: Vec<Card> = first.cards().iter().take(4).cloned().collect();
    let mut corrupted = rows.clone();
    if let Some(slot) = corrupted.first_mut() {
        *slot = JokerVectorRow::from_parts(
            cards,
            first.expected_class(),
            first.expected_tiebreak().to_vec(),
        );
    }
    corrupted
}

fn cyclic_hand(index: u8) -> EvaluatedHand {
    EvaluatedHand::new(HandClass::HighCard, vec![5, index], Vec::new(), Vec::new())
}

fn cyclic_compare(left: &EvaluatedHand, right: &EvaluatedHand) -> Ordering {
    let left = left.tiebreak().get(1).copied().unwrap_or(0) % 3;
    let right = right.tiebreak().get(1).copied().unwrap_or(0) % 3;
    if left == right {
        Ordering::Equal
    } else if (left + 1) % 3 == right {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}

#[test]
fn test_self_test_runs_six_checks_with_the_frozen_names() {
    // Act
    let report = self_test();
    let names: Vec<&str> = report.checks.iter().map(|check| check.name).collect();

    // Assert
    assert_eq!(
        names,
        vec![
            "7,462 distinct ranks",
            "Joker vector table (28)",
            "Five-of-a-kind ordering",
            "Whole-field 180×12 timing",
            "compare() total order",
            "best_hand() joker bound",
        ]
    );
}

#[test]
fn test_self_test_marks_the_timing_check_advisory() {
    // Act
    let report = self_test();
    let blocking: Vec<bool> = report.checks.iter().map(|check| check.blocking).collect();

    // Assert
    assert_eq!(blocking, vec![true, true, true, false, true, true]);
}

#[test]
fn test_self_test_records_each_checks_duration() {
    // Act
    let report = self_test();
    let measured: Vec<bool> = report
        .checks
        .iter()
        .map(|check| check.duration_seconds > 0.0)
        .collect();

    // Assert
    assert_eq!(measured, vec![false, false, false, true, false, false]);
}

#[test]
fn test_self_test_passes_on_the_committed_vectors() {
    // Act
    let report = self_test();
    let passed: Vec<bool> = report.checks.iter().map(|check| check.passed).collect();

    // Assert
    assert_eq!(passed, vec![true, true, true, true, true, true]);
}

#[test]
fn test_self_test_fails_gracefully_when_a_vector_row_is_corrupted() {
    // Arrange
    let sweep = rank_rows_with_a_wrong_rank();
    let jokers = joker_rows_with_a_wrong_class();

    // Act
    let report = build_report(&sweep, &jokers, compare, six_joker_hand, Instant::now);
    let failed: Vec<&str> = report
        .checks
        .iter()
        .filter(|check| !check.passed)
        .map(|check| check.name)
        .collect();

    // Assert
    assert_eq!(
        failed,
        vec![
            "7,462 distinct ranks",
            "Joker vector table (28)",
            "best_hand() joker bound",
        ]
    );
}

#[test]
fn test_whole_field_meets_the_requirement() {
    // Arrange
    let field = seeded_field();

    // Act
    let start = Instant::now();
    let hands: Vec<EvaluatedHand> = field.iter().map(|entry| best_hand(entry)).collect();
    let elapsed = start.elapsed().as_secs_f64();

    // Assert
    assert_eq!(hands.len(), 180);
    assert!(elapsed < 1.0, "the 180x12 field took {elapsed:.3} s");
}

#[test]
fn test_self_test_marks_a_slow_field_as_advisory() {
    // Act
    let report = build_report(
        &committed_rank_rows(),
        &committed_joker_rows(),
        compare,
        best_hand,
        slow_clock,
    );
    let check = report.checks.get(3).expect("the report holds six checks");

    // Assert
    assert_eq!(
        (check.passed, check.blocking, check.duration_seconds > 1.0),
        (false, false, true)
    );
}

#[test]
fn test_self_test_seeded_field_holds_180_entries_of_12_cards() {
    // Act
    let field = seeded_field();
    let all_twelve = field.iter().all(|entry| entry.len() == 12);

    // Assert
    assert_eq!((field.len(), all_twelve), (180, true));
}

#[test]
fn test_self_test_seeded_field_is_deterministic() {
    // Act
    let first = seeded_field();
    let second = seeded_field();

    // Assert
    assert_eq!(first, second);
}

#[test]
fn test_self_test_seeded_compare_sample_holds_48_hands() {
    // Act
    let sample = seeded_compare_sample();
    let jokers = sample
        .iter()
        .filter(|hand| !hand.jokers_played_as().is_empty())
        .count();

    // Assert
    assert_eq!((sample.len(), jokers), (48, 16));
}

#[test]
fn test_self_test_fails_rank_sweep_on_a_short_row() {
    // Arrange
    let sweep = rank_rows_with_a_short_row();

    // Act
    let report = build_report(
        &sweep,
        &committed_joker_rows(),
        compare,
        best_hand,
        Instant::now,
    );
    let check = report.checks.first().expect("the report holds six checks");

    // Assert
    assert_eq!(
        (check.passed, check.detail.as_str()),
        (false, "a sweep row does not hold 5 cards")
    );
}

#[test]
fn test_self_test_fails_rank_sweep_on_duplicate_keys() {
    // Arrange
    let sweep = rank_rows_with_duplicate_keys();

    // Act
    let report = build_report(
        &sweep,
        &committed_joker_rows(),
        compare,
        best_hand,
        Instant::now,
    );
    let check = report.checks.first().expect("the report holds six checks");

    // Assert
    assert_eq!(
        (check.passed, check.detail.as_str()),
        (false, "got 7461 distinct ranks")
    );
}

#[test]
fn test_self_test_fails_rank_sweep_on_a_short_row_set() {
    // Arrange
    let rows = committed_rank_rows();
    let short: Vec<RankSweepRow> = rows.get(1..).unwrap_or_default().to_vec();

    // Act
    let report = build_report(
        &short,
        &committed_joker_rows(),
        compare,
        best_hand,
        Instant::now,
    );
    let check = report.checks.first().expect("the report holds six checks");

    // Assert
    assert_eq!(
        (check.passed, check.detail.as_str()),
        (false, "got 7461 rows")
    );
}

#[test]
fn test_self_test_fails_joker_vectors_on_a_short_row() {
    // Arrange
    let jokers = joker_rows_with_a_short_row();

    // Act
    let report = build_report(
        &committed_rank_rows(),
        &jokers,
        compare,
        best_hand,
        Instant::now,
    );
    let check = report.checks.get(1).expect("the report holds six checks");

    // Assert
    assert_eq!(
        (check.passed, check.detail.as_str()),
        (false, "a joker row does not hold 5 cards")
    );
}

#[test]
fn test_self_test_fails_joker_vectors_on_a_short_row_set() {
    // Arrange
    let rows = committed_joker_rows();
    let short: Vec<JokerVectorRow> = rows.get(1..).unwrap_or_default().to_vec();

    // Act
    let report = build_report(
        &committed_rank_rows(),
        &short,
        compare,
        best_hand,
        Instant::now,
    );
    let check = report.checks.get(1).expect("the report holds six checks");

    // Assert
    assert_eq!(
        (check.passed, check.detail.as_str()),
        (false, "got 27 rows")
    );
}

#[test]
fn test_self_test_fails_joker_vectors_on_a_wrong_tiebreak() {
    // Arrange
    let jokers = joker_rows_with_a_wrong_tiebreak();

    // Act
    let report = build_report(
        &committed_rank_rows(),
        &jokers,
        compare,
        best_hand,
        Instant::now,
    );
    let check = report.checks.get(1).expect("the report holds six checks");

    // Assert
    assert_eq!(
        (check.passed, check.detail.as_str()),
        (false, "test row: tiebreak does not match")
    );
}

#[test]
fn test_self_test_fails_five_of_a_kind_ordering_on_a_wrong_compare() {
    // Act
    let report = build_report(
        &committed_rank_rows(),
        &committed_joker_rows(),
        always_less,
        best_hand,
        Instant::now,
    );
    let check = report.checks.get(2).expect("the report holds six checks");

    // Assert
    assert_eq!(
        (check.passed, check.detail.as_str()),
        (false, "five of a kind must beat a royal flush")
    );
}

#[test]
fn test_self_test_fails_compare_total_order_on_a_wrong_compare() {
    // Act
    let report = build_report(
        &committed_rank_rows(),
        &committed_joker_rows(),
        always_greater,
        best_hand,
        Instant::now,
    );
    let check = report.checks.get(4).expect("the report holds six checks");

    // Assert
    assert_eq!(
        (check.passed, check.detail.as_str()),
        (false, "antisymmetric, reflexive, transitive failed")
    );
}

#[test]
fn test_compare_total_order_fails_transitivity_on_a_cyclic_relation() {
    // Arrange
    let sample = vec![cyclic_hand(0), cyclic_hand(1), cyclic_hand(2)];

    // Act
    let verdict = compare_total_order(&sample, cyclic_compare);

    // Assert
    assert_eq!(
        (verdict.passed, verdict.detail.as_str()),
        (false, "transitive failed")
    );
}
