// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the self-test dialog's dotted canvas lines.

use pretty_assertions::assert_eq;
use rivercrossing_hands::self_test::{SelfTestCheck, SelfTestReport};

use super::{format_check_line, format_report, run};

/// Build one self-test check.
fn check(name: &'static str, passed: bool, detail: &str) -> SelfTestCheck {
    SelfTestCheck {
        name,
        passed,
        duration_seconds: 0.0,
        detail: detail.to_owned(),
        blocking: true,
    }
}

// ------------------------------------------------------- frozen lines

#[test]
fn test_selftest_format_check_line_given_the_rank_sweep_matches_the_frozen_line() {
    // Arrange
    let sweep = check("7,462 distinct ranks", true, "");

    // Act
    let line = format_check_line(&sweep);

    // Assert
    assert_eq!(line, "7,462 distinct ranks ........ PASS");
}

#[test]
fn test_selftest_format_check_line_given_the_joker_vectors_matches_the_frozen_line() {
    // Arrange
    let vectors = check("Joker vector table (28)", true, "");

    // Act
    let line = format_check_line(&vectors);

    // Assert
    assert_eq!(line, "Joker vector table (28) ..... PASS");
}

#[test]
fn test_selftest_format_check_line_given_the_ordering_check_matches_the_frozen_line() {
    // Arrange
    let ordering = check("Five-of-a-kind ordering", true, "");

    // Act
    let line = format_check_line(&ordering);

    // Assert
    assert_eq!(line, "Five-of-a-kind ordering ..... PASS");
}

#[test]
fn test_selftest_format_check_line_given_the_timing_detail_matches_the_frozen_line() {
    // Arrange
    let timing = check("Whole-field 180×12 timing", true, "0.31 s");

    // Act
    let line = format_check_line(&timing);

    // Assert
    assert_eq!(line, "Whole-field 180×12 timing ... 0.31 s PASS");
}

#[test]
fn test_selftest_format_check_line_given_the_total_order_check_matches_the_frozen_line() {
    // Arrange
    let total_order = check("compare() total order", true, "");

    // Act
    let line = format_check_line(&total_order);

    // Assert
    assert_eq!(line, "compare() total order ....... PASS");
}

#[test]
fn test_selftest_format_check_line_given_the_joker_bound_check_matches_the_frozen_line() {
    // Arrange
    let joker_bound = check("best_hand() joker bound", true, "");

    // Act
    let line = format_check_line(&joker_bound);

    // Assert
    assert_eq!(line, "best_hand() joker bound ..... PASS");
}

// ------------------------------------------------------- PASS and FAIL

#[test]
fn test_selftest_format_check_line_when_failed_renders_the_fail_suffix() {
    // Arrange
    let failed = check("7,462 distinct ranks", false, "");

    // Act
    let line = format_check_line(&failed);

    // Assert
    assert_eq!(line, "7,462 distinct ranks ........ FAIL");
}

#[test]
fn test_selftest_format_check_line_when_a_failing_advisory_check_renders_fail() {
    // Arrange
    let advisory = SelfTestCheck {
        name: "Whole-field 180×12 timing",
        passed: false,
        duration_seconds: 0.0,
        detail: "9.99 s".to_owned(),
        blocking: false,
    };

    // Act
    let line = format_check_line(&advisory);

    // Assert
    assert_eq!(line, "Whole-field 180×12 timing ... 9.99 s FAIL");
}

#[test]
fn test_selftest_format_check_line_given_a_passing_detail_keeps_the_detail() {
    // Arrange
    let passing_detail = check("Whole-field 180×12 timing", true, "0.12 s");

    // Act
    let line = format_check_line(&passing_detail);

    // Assert
    assert_eq!(line, "Whole-field 180×12 timing ... 0.12 s PASS");
}

#[test]
fn test_selftest_format_check_line_given_the_domain_pass_placeholder_drops_it() {
    // Arrange: the hands crate stores `ok` as a passing check's detail.
    let placeholder = check("7,462 distinct ranks", true, "ok");

    // Act
    let line = format_check_line(&placeholder);

    // Assert
    assert_eq!(line, "7,462 distinct ranks ........ PASS");
}

#[test]
fn test_selftest_format_check_line_given_a_name_at_the_column_has_no_dots() {
    // Arrange: 28 characters plus one space fills the dotted column.
    let at_column = check("1234567890123456789012345678", true, "");

    // Act
    let line = format_check_line(&at_column);

    // Assert
    assert_eq!(line, "1234567890123456789012345678  PASS");
}

#[test]
fn test_selftest_format_check_line_given_a_longer_name_keeps_it_whole() {
    // Arrange
    let longer = check("12345678901234567890123456789", true, "");

    // Act
    let line = format_check_line(&longer);

    // Assert
    assert_eq!(line, "12345678901234567890123456789  PASS");
}

// ------------------------------------------------------- format_report

#[test]
fn test_selftest_format_report_renders_one_line_per_check_in_order() {
    // Arrange
    let report = SelfTestReport {
        checks: vec![check("a", true, ""), check("b", false, "")],
    };

    // Act
    let lines = format_report(&report);

    // Assert
    assert_eq!(
        lines,
        vec![
            "a ........................... PASS".to_owned(),
            "b ........................... FAIL".to_owned(),
        ]
    );
}

#[test]
fn test_selftest_format_report_given_no_checks_returns_no_lines() {
    // Arrange
    let report = SelfTestReport { checks: Vec::new() };

    // Act
    let lines = format_report(&report);

    // Assert
    assert_eq!(lines, Vec::<String>::new());
}

// ---------------------------------------------------------------- run

#[test]
fn test_selftest_run_runs_the_six_checks_in_their_frozen_order() {
    // Arrange: no injectable seam; the suite is deterministic.

    // Act
    let lines = run();

    // Assert
    assert_eq!(lines.len(), 6);
    assert_eq!(
        lines.first().map(String::as_str),
        Some("7,462 distinct ranks ........ PASS")
    );
    assert_eq!(
        lines.get(1).map(String::as_str),
        Some("Joker vector table (28) ..... PASS")
    );
    assert_eq!(
        lines.get(2).map(String::as_str),
        Some("Five-of-a-kind ordering ..... PASS")
    );
    assert_eq!(
        lines.get(4).map(String::as_str),
        Some("compare() total order ....... PASS")
    );
    assert_eq!(
        lines.get(5).map(String::as_str),
        Some("best_hand() joker bound ..... PASS")
    );
}

#[test]
fn test_selftest_run_given_the_timing_check_renders_its_measured_detail() {
    // Arrange: the timing line is the one line with a detail.

    // Act
    let lines = run();

    // Assert
    assert_eq!(
        lines
            .get(3)
            .map(|line| line.starts_with("Whole-field 180×12 timing ... ")),
        Some(true)
    );
    assert_eq!(lines.get(3).map(|line| line.ends_with(" PASS")), Some(true));
}
