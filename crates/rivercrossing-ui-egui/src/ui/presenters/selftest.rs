// SPDX-License-Identifier: GPL-3.0-only

//! The self-test dialog's dotted canvas lines, with no rendering.
//!
//! The rules follow the Python oracle's `ui/presenters/selftest.py`: every
//! check's name is padded with dots to a fixed column, then the check's own
//! detail when it carries one, then `PASS` or `FAIL`. The suite is the live
//! [`rivercrossing_hands::self_test`] hook, so the dialog shows the real
//! evaluator result rather than display data.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the EPIC-12c views wire these decisions in a later phase; phase 1 unit-tests them"
    )
)]

use rivercrossing_hands::self_test::{SelfTestCheck, SelfTestReport};

#[cfg(test)]
mod tests;

/// The canvas column that every check's `PASS`/`FAIL` lines up on.
const DOT_COLUMN: usize = 29;

/// The hands crate's placeholder detail for a passing check.
///
/// The canvas shows no detail for a passing check, so the placeholder is not
/// rendered.
const PASS_PLACEHOLDER: &str = "ok";

/// Render one check as the canvas's dotted `PASS`/`FAIL` line.
///
/// The name and one space are padded with dots to [`DOT_COLUMN`], then a
/// space, the check's detail when it has one, and its verdict.
#[must_use]
pub(crate) fn format_check_line(check: &SelfTestCheck) -> String {
    let label = dotted_label(check.name);
    let detail = detail_text(check);
    let status = if check.passed { "PASS" } else { "FAIL" };
    format!("{label} {detail}{status}")
}

/// Return the check's name padded with dots to the canvas column.
fn dotted_label(name: &str) -> String {
    let mut label = format!("{name} ");
    let width = label.chars().count();
    if width < DOT_COLUMN {
        label.push_str(&".".repeat(DOT_COLUMN - width));
    }
    label
}

/// Return the check's detail and its trailing space, or nothing.
fn detail_text(check: &SelfTestCheck) -> String {
    if check.detail.is_empty() || (check.passed && check.detail == PASS_PLACEHOLDER) {
        String::new()
    } else {
        format!("{} ", check.detail)
    }
}

/// Render every check in *report* as its own dotted canvas line.
#[must_use]
pub(crate) fn format_report(report: &SelfTestReport) -> Vec<String> {
    report.checks.iter().map(format_check_line).collect()
}

/// Run the evaluator self-test suite and return its canvas lines.
#[must_use]
pub(crate) fn run() -> Vec<String> {
    format_report(&rivercrossing_hands::self_test::self_test())
}
