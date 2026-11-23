// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the rider-issue type.

use pretty_assertions::assert_eq;
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::types::{Entry, Rider};

use super::{IssueKind, RiderIssue};

/// Return one solo entry with a fully formed rider.
fn entry() -> Entry {
    Entry::restored(
        "key-1",
        "7",
        "Sam Ellis",
        EntryType::Solo,
        vec![Rider {
            first_name: "Sam".to_owned(),
            last_name: "Ellis".to_owned(),
            plate: Some("7".to_owned()),
            sex: None,
            sort_order: 0,
        }],
    )
}

#[test]
fn test_issue_kind_given_the_same_variant_compares_equal() {
    assert_eq!(IssueKind::DuplicateNumber, IssueKind::DuplicateNumber);
}

#[test]
fn test_issue_kind_given_different_variants_compares_unequal() {
    assert_ne!(IssueKind::TeamOfOne, IssueKind::NearDuplicateTeamName);
}

#[test]
fn test_rider_issue_given_a_rider_reports_every_field() {
    // Arrange
    let entry = entry();
    let issue = RiderIssue {
        entry: &entry,
        rider: entry.riders.first(),
        kind: IssueKind::DuplicateName,
        message: "duplicate rider name Sam Ellis".to_owned(),
    };

    // Assert
    assert_eq!(issue.kind, IssueKind::DuplicateName);
    assert_eq!(issue.message, "duplicate rider name Sam Ellis");
    assert_eq!(issue.entry.key, "key-1");
    assert_eq!(
        issue.rider.map(Rider::full_name),
        Some("Sam Ellis".to_owned())
    );
}

#[test]
fn test_rider_issue_given_a_team_scoped_issue_reports_no_rider() {
    // Arrange
    let entry = entry();
    let issue = RiderIssue {
        entry: &entry,
        rider: None,
        kind: IssueKind::TeamOfOne,
        message: "team size must be at least 2, got 1".to_owned(),
    };

    // Assert
    assert_eq!(issue.kind, IssueKind::TeamOfOne);
    assert_eq!(issue.message, "team size must be at least 2, got 1");
    assert_eq!(issue.entry.key, "key-1");
    assert_eq!(issue.rider, None);
}
