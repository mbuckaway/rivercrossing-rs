// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the roster checks.

use pretty_assertions::assert_eq;
use rivercrossing_roster::enums::{EntryMode, EntryStatus, EntryType, PlateModel};
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::{Entry, Rider};
use rstest::rstest;

use super::rider_issues;
use crate::issue::{IssueKind, RiderIssue};

/// Return a mixed ride under the pooled plate model.
fn pooled_roster() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(8843))
        .expect("the test roster settings are valid")
}

/// Return a mixed ride under the relay plate model.
fn relay_roster() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::TeamRelay, Some(8843))
        .expect("the test roster settings are valid")
}

/// Return one rider with the given name and plate.
fn rider(first_name: &str, last_name: &str, plate: Option<&str>) -> Rider {
    Rider {
        first_name: first_name.to_owned(),
        last_name: last_name.to_owned(),
        plate: plate.map(str::to_owned),
        sex: None,
        sort_order: 0,
    }
}

/// Return one solo entry holding *rider*, named for that rider.
fn solo(key: &str, plate: &str, rider: Rider) -> Entry {
    let display_name = rider.full_name();
    Entry::restored(key, plate, display_name, EntryType::Solo, vec![rider])
}

/// Return one team entry holding *riders*, plated for its first rider.
fn team(key: &str, display_name: &str, riders: Vec<Rider>) -> Entry {
    let plate = riders
        .first()
        .and_then(|rider| rider.plate.clone())
        .unwrap_or_default();
    Entry::restored(key, plate, display_name, EntryType::Team, riders)
}

/// Return *count* distinct, fully named riders plated `{prefix}1`.. .
fn team_riders(count: usize, prefix: &str) -> Vec<Rider> {
    (1..=count)
        .map(|index| {
            rider(
                &format!("{prefix} Rider {index}"),
                "Team",
                Some(&format!("{prefix}{index}")),
            )
        })
        .collect()
}

/// Return the report's only issue, asserting that it holds exactly one.
fn single_issue(roster: &Roster) -> RiderIssue<'_> {
    let issues = rider_issues(roster);
    assert_eq!(issues.len(), 1);
    issues
        .into_iter()
        .next()
        .expect("the report holds one issue")
}

// ------------------------------------------------------------ empty roster

#[test]
fn test_rider_issues_given_an_empty_roster_reports_nothing() {
    assert_eq!(rider_issues(&pooled_roster()).len(), 0);
}

// ------------------------------------------------------------- team of one

#[rstest]
#[case(0, "team size must be at least 2, got 0")]
#[case(1, "team size must be at least 2, got 1")]
fn test_rider_issues_given_an_undersized_team_reports_team_of_one(
    #[case] size: usize,
    #[case] message: &str,
) {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![team("k1", "Lone Wolf", team_riders(size, "1"))]);

    // Act
    let issue = single_issue(&roster);

    // Assert
    assert_eq!(issue.kind, IssueKind::TeamOfOne);
    assert_eq!(issue.message, message);
    assert_eq!(issue.entry.key, "k1");
    assert_eq!(issue.rider, None);
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn test_rider_issues_given_a_valid_size_team_reports_no_team_of_one(#[case] size: usize) {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![team("k1", "Trail Blazers", team_riders(size, "1"))]);

    // Assert
    assert_eq!(rider_issues(&roster).len(), 0);
}

// ------------------------------------------------------------ missing name

#[rstest]
#[case("", "")]
#[case("   ", "")]
#[case("", "\t")]
#[case("  ", "   ")]
fn test_rider_issues_given_blank_name_parts_reports_missing_name(
    #[case] first: &str,
    #[case] last: &str,
) {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![solo("k1", "1", rider(first, last, Some("1")))]);

    // Act
    let issue = single_issue(&roster);

    // Assert
    assert_eq!(issue.kind, IssueKind::MissingName);
    assert_eq!(issue.message, "missing name");
    assert_eq!(issue.entry.key, "k1");
    assert_eq!(issue.rider.map(Rider::full_name), Some(String::new()));
}

// ---------------------------------------------------------- missing number

#[rstest]
#[case(None)]
#[case(Some(""))]
#[case(Some("   "))]
fn test_rider_issues_given_a_pooled_rider_with_a_blank_plate_reports_missing_number(
    #[case] plate: Option<&str>,
) {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![solo("k1", "", rider("Alex", "Roy", plate))]);

    // Act
    let issue = single_issue(&roster);

    // Assert
    assert_eq!(issue.kind, IssueKind::MissingNumber);
    assert_eq!(issue.message, "missing plate");
    assert_eq!(issue.entry.key, "k1");
    assert_eq!(
        issue.rider.map(Rider::full_name),
        Some("Alex Roy".to_owned())
    );
}

#[test]
fn test_rider_issues_given_a_relay_rider_without_a_plate_reports_nothing() {
    // Arrange
    let mut roster = relay_roster();
    roster.load_entries(vec![solo("k1", "7", rider("Alex", "Roy", None))]);

    // Assert
    assert_eq!(rider_issues(&roster).len(), 0);
}

// --------------------------------------------------------- duplicate names

#[test]
fn test_rider_issues_given_a_case_and_whitespace_duplicate_name_reports_duplicate_name() {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![
        solo("k1", "1", rider("Mary Anne", "Knibbe", Some("1"))),
        solo("k2", "2", rider("  mary   anne ", " KNIBBE ", Some("2"))),
    ]);

    // Act
    let issue = single_issue(&roster);

    // Assert
    assert_eq!(issue.kind, IssueKind::DuplicateName);
    assert_eq!(issue.message, "duplicate rider name mary   anne KNIBBE");
    assert_eq!(issue.entry.key, "k2");
    assert_eq!(
        issue.rider.map(Rider::full_name),
        Some("mary   anne KNIBBE".to_owned())
    );
}

// ---------------------------------------------------- duplicate team names

#[test]
fn test_rider_issues_given_two_teams_sharing_a_normalized_name_reports_duplicate_team_name() {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![
        team("k1", "Thunder", team_riders(2, "20")),
        team("k2", "  thunder ", team_riders(2, "23")),
    ]);

    // Act
    let issue = single_issue(&roster);

    // Assert
    assert_eq!(issue.kind, IssueKind::DuplicateTeamName);
    assert_eq!(issue.message, "duplicate team name \"  thunder \"");
    assert_eq!(issue.entry.key, "k2");
    assert_eq!(issue.rider, None);
}

#[test]
fn test_rider_issues_given_a_single_team_reports_no_team_name_issue() {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![team("k1", "Thunder", team_riders(2, "20"))]);

    // Assert
    assert_eq!(rider_issues(&roster).len(), 0);
}

#[test]
fn test_rider_issues_given_distinct_team_names_reports_no_team_name_issue() {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![
        team("k1", "Thunder", team_riders(2, "20")),
        team("k2", "Lightning", team_riders(2, "23")),
    ]);

    // Assert
    assert_eq!(rider_issues(&roster).len(), 0);
}

#[test]
fn test_rider_issues_given_duplicate_solo_display_names_reports_no_team_name_issue() {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![
        Entry::restored(
            "k1",
            "1",
            "Same",
            EntryType::Solo,
            vec![rider("Alex", "Roy", Some("1"))],
        ),
        Entry::restored(
            "k2",
            "2",
            "Same",
            EntryType::Solo,
            vec![rider("Sam", "Ellis", Some("2"))],
        ),
    ]);

    // Assert
    assert_eq!(rider_issues(&roster).len(), 0);
}

#[test]
fn test_rider_issues_given_a_hard_team_duplicate_reports_no_near_duplicate() {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![
        team("k1", "Thunder", team_riders(2, "20")),
        team("k2", "thunder", team_riders(2, "23")),
    ]);

    // Act
    let kinds: Vec<IssueKind> = rider_issues(&roster)
        .iter()
        .map(|issue| issue.kind)
        .collect();

    // Assert
    assert_eq!(kinds, [IssueKind::DuplicateTeamName]);
}

// ----------------------------------------------- near-duplicate team names

#[test]
fn test_rider_issues_given_near_duplicate_team_names_reports_the_prefixed_warning() {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![
        team("k1", "Good 2 Go", team_riders(2, "30")),
        team("k2", "Good 2Go", team_riders(2, "33")),
    ]);

    // Act
    let issue = single_issue(&roster);

    // Assert
    assert_eq!(issue.kind, IssueKind::NearDuplicateTeamName);
    assert_eq!(
        issue.message,
        "⚠ warning: possible duplicate team name: \"good 2 go\" and \"good 2go\""
    );
    assert_eq!(issue.entry.key, "k2");
    assert_eq!(issue.rider, None);
}

// ------------------------------------------------------- duplicate numbers

#[test]
fn test_rider_issues_given_two_pooled_riders_sharing_a_plate_reports_duplicate_number() {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![
        solo("k1", "7", rider("Sam", "Ellis", Some("7"))),
        solo("k2", "7", rider("Alex", "Roy", Some("7"))),
    ]);

    // Act
    let issue = single_issue(&roster);

    // Assert
    assert_eq!(issue.kind, IssueKind::DuplicateNumber);
    assert_eq!(issue.message, "duplicate plate 7");
    assert_eq!(issue.entry.key, "k2");
    assert_eq!(
        issue.rider.map(Rider::full_name),
        Some("Alex Roy".to_owned())
    );
}

#[test]
fn test_rider_issues_given_two_relay_entries_sharing_a_plate_reports_duplicate_number() {
    // Arrange
    let mut roster = relay_roster();
    roster.load_entries(vec![
        solo("k1", "7", rider("Sam", "Ellis", None)),
        solo("k2", "7", rider("Alex", "Roy", None)),
    ]);

    // Act
    let issue = single_issue(&roster);

    // Assert
    assert_eq!(issue.kind, IssueKind::DuplicateNumber);
    assert_eq!(issue.message, "duplicate plate 7");
    assert_eq!(issue.entry.key, "k2");
    assert_eq!(issue.rider, None);
}

#[test]
fn test_rider_issues_given_a_relay_blank_entry_plate_reports_nothing() {
    // Arrange
    let mut roster = relay_roster();
    roster.load_entries(vec![solo("k1", "   ", rider("Sam", "Ellis", None))]);

    // Assert
    assert_eq!(rider_issues(&roster).len(), 0);
}

// ------------------------------------------------------- status and order

#[test]
fn test_rider_issues_given_a_dnf_one_rider_team_still_reports_team_of_one() {
    // Arrange
    let mut roster = pooled_roster();
    let mut entry = team("k1", "Lone Wolf", team_riders(1, "1"));
    entry.status = EntryStatus::Dnf;
    roster.load_entries(vec![entry]);

    // Act
    let issue = single_issue(&roster);

    // Assert
    assert_eq!(issue.kind, IssueKind::TeamOfOne);
    assert_eq!(issue.message, "team size must be at least 2, got 1");
    assert_eq!(issue.entry.key, "k1");
}

#[test]
fn test_rider_issues_given_every_kind_reports_them_in_canonical_order() {
    // Arrange
    let mut roster = pooled_roster();
    roster.load_entries(vec![
        team("k1", "Lone Wolf", team_riders(1, "1")),
        solo("k2", "2", rider("", "", Some("2"))),
        solo("k3", "", rider("No", "Plate", None)),
        solo("k4", "5", rider("Mary Anne", "Knibbe", Some("5"))),
        solo("k5", "5", rider("mary anne", "knibbe", Some("5"))),
        team("k6", "Thunder", team_riders(2, "20")),
        team("k7", "thunder", team_riders(2, "23")),
        team("k8", "Good 2 Go", team_riders(2, "30")),
        team("k9", "Good 2Go", team_riders(2, "33")),
    ]);

    // Act
    let kinds: Vec<IssueKind> = rider_issues(&roster)
        .iter()
        .map(|issue| issue.kind)
        .collect();

    // Assert
    assert_eq!(
        kinds,
        [
            IssueKind::TeamOfOne,
            IssueKind::MissingName,
            IssueKind::MissingNumber,
            IssueKind::DuplicateName,
            IssueKind::DuplicateTeamName,
            IssueKind::NearDuplicateTeamName,
            IssueKind::DuplicateNumber,
        ]
    );
}
