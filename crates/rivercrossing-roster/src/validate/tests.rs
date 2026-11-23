// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the start gate's team-size rule.

use pretty_assertions::assert_eq;
use rivercrossing_ride::StartViolation;

use crate::enums::{EntryMode, EntryType, PlateModel};
use crate::roster::{Roster, TeamFields};
use crate::types::{Entry, EntryId, Rider};

fn pooled() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, None)
        .expect("the default settings are valid")
}

fn relay() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::TeamRelay, None)
        .expect("the default settings are valid")
}

fn named(first_name: &str, plate: Option<&str>) -> Rider {
    Rider {
        first_name: first_name.to_owned(),
        plate: plate.map(str::to_owned),
        ..Rider::default()
    }
}

fn pooled_team(roster: &mut Roster, display_name: &str, members: &[(&str, &str)]) -> EntryId {
    let riders = members
        .iter()
        .map(|(first_name, plate)| named(first_name, Some(plate)))
        .collect();
    roster
        .create_team_entry(TeamFields {
            display_name,
            riders,
            ..TeamFields::default()
        })
        .expect("a valid team is accepted")
}

fn relay_team(roster: &mut Roster, display_name: &str, plate: &str) -> EntryId {
    roster
        .create_team_entry(TeamFields {
            display_name,
            riders: vec![named("Alex", None), named("Bo", None)],
            plate: Some(plate),
            ..TeamFields::default()
        })
        .expect("a valid team is accepted")
}

fn pooled_team_of_one(roster: &mut Roster, display_name: &str, rider_plate: &str) -> EntryId {
    roster
        .create_team_entry_of_one(TeamFields {
            display_name,
            riders: vec![named("Alex", Some(rider_plate))],
            ..TeamFields::default()
        })
        .expect("a draft team of one is accepted")
}

fn relay_team_of_one(roster: &mut Roster, display_name: &str, plate: &str) -> EntryId {
    roster
        .create_team_entry_of_one(TeamFields {
            display_name,
            riders: vec![named("Alex", None)],
            plate: Some(plate),
            ..TeamFields::default()
        })
        .expect("a draft team of one is accepted")
}

fn pooled_empty_team(roster: &mut Roster, display_name: &str) -> EntryId {
    roster
        .create_empty_team(TeamFields {
            display_name,
            ..TeamFields::default()
        })
        .expect("a draft empty team is accepted")
}

fn relay_empty_team(roster: &mut Roster, display_name: &str, plate: &str) -> EntryId {
    roster
        .create_empty_team(TeamFields {
            display_name,
            plate: Some(plate),
            ..TeamFields::default()
        })
        .expect("a draft empty team is accepted")
}

fn solo(roster: &mut Roster, first_name: &str, plate: &str) -> EntryId {
    roster
        .create_solo_entry(named(first_name, None), plate)
        .expect("a fresh plate is accepted")
}

fn retired_team(key: &str, plate: &str) -> Entry {
    Entry::restored(
        key,
        plate,
        "Retired",
        EntryType::Team,
        vec![named("Alex", None)],
    )
}

fn violation(plate: &str, size: usize) -> StartViolation {
    StartViolation {
        plate: plate.to_owned(),
        reason: format!("team size must be at least 2, got {size}"),
    }
}

// --------------------------------------------------------------- clean cases

#[test]
fn test_validate_for_start_given_an_empty_roster_returns_no_violations() {
    let roster = pooled();

    assert_eq!(roster.validate_for_start(), Vec::<StartViolation>::new());
}

#[test]
fn test_validate_for_start_given_only_solos_returns_no_violations() {
    let mut roster = pooled();
    solo(&mut roster, "Ada", "1");
    solo(&mut roster, "Bo", "2");

    assert_eq!(roster.validate_for_start(), Vec::<StartViolation>::new());
}

#[test]
fn test_validate_for_start_given_a_two_rider_team_returns_no_violations() {
    let mut roster = pooled();
    pooled_team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    assert_eq!(roster.validate_for_start(), Vec::<StartViolation>::new());
}

#[test]
fn test_validate_for_start_given_a_max_size_team_returns_no_violations() {
    let mut roster = pooled();
    pooled_team(
        &mut roster,
        "Team A",
        &[("Alex", "1"), ("Bo", "2"), ("Cy", "3"), ("Do", "4")],
    );

    assert_eq!(roster.validate_for_start(), Vec::<StartViolation>::new());
}

// ------------------------------------------------------- small team reports

#[test]
fn test_validate_for_start_given_a_pooled_team_of_one_reports_the_exact_reason() {
    let mut roster = pooled();
    pooled_team_of_one(&mut roster, "Team A", "5");

    assert_eq!(roster.validate_for_start(), [violation("5", 1)]);
}

#[test]
fn test_validate_for_start_given_a_relay_team_of_one_reports_the_exact_reason() {
    let mut roster = relay();
    relay_team_of_one(&mut roster, "Team A", "7");

    assert_eq!(roster.validate_for_start(), [violation("7", 1)]);
}

#[test]
fn test_validate_for_start_given_a_pooled_empty_team_reports_the_exact_reason() {
    let mut roster = pooled();
    pooled_empty_team(&mut roster, "Team A");

    assert_eq!(roster.validate_for_start(), [violation("1", 0)]);
}

#[test]
fn test_validate_for_start_given_a_relay_empty_team_reports_the_exact_reason() {
    let mut roster = relay();
    relay_empty_team(&mut roster, "Team A", "9");

    assert_eq!(roster.validate_for_start(), [violation("9", 0)]);
}

// ------------------------------------------------------------- ordering

#[test]
fn test_validate_for_start_given_two_small_teams_reports_them_in_creation_order() {
    let mut roster = pooled();
    pooled_team_of_one(&mut roster, "First", "3");
    pooled_team_of_one(&mut roster, "Second", "4");

    assert_eq!(
        roster.validate_for_start(),
        [violation("3", 1), violation("4", 1)]
    );
}

#[test]
fn test_validate_for_start_given_a_clean_team_between_small_teams_skips_it() {
    let mut roster = relay();
    relay_team_of_one(&mut roster, "First", "3");
    relay_team(&mut roster, "Clean", "4");
    relay_empty_team(&mut roster, "Third", "9");

    assert_eq!(
        roster.validate_for_start(),
        [violation("3", 1), violation("9", 0)]
    );
}

#[test]
fn test_validate_for_start_given_a_solo_between_small_teams_skips_it() {
    let mut roster = pooled();
    pooled_team_of_one(&mut roster, "First", "3");
    solo(&mut roster, "Ada", "5");
    pooled_team_of_one(&mut roster, "Second", "4");

    assert_eq!(
        roster.validate_for_start(),
        [violation("3", 1), violation("4", 1)]
    );
}

// --------------------------------------------------------------- retired

#[test]
fn test_validate_for_start_given_only_a_retired_small_team_returns_no_violations() {
    let mut roster = pooled();
    roster.load_retired_entries(vec![retired_team("r1", "9")]);

    assert_eq!(roster.validate_for_start(), Vec::<StartViolation>::new());
}

#[test]
fn test_validate_for_start_given_a_retired_small_team_ignores_it() {
    let mut roster = pooled();
    pooled_team_of_one(&mut roster, "Live", "3");
    roster.load_retired_entries(vec![retired_team("r1", "9")]);

    assert_eq!(roster.validate_for_start(), [violation("3", 1)]);
}

// ------------------------------------------------- entry plate, not rider

#[test]
fn test_validate_for_start_given_a_team_reduced_to_one_reports_the_recomputed_plate() {
    let mut roster = pooled();
    let source = pooled_team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let destination = pooled_team(&mut roster, "Team B", &[("Cy", "10"), ("Do", "11")]);
    roster
        .move_rider("1", &destination, "rider swapped teams")
        .expect("the move is allowed");

    assert_eq!(roster.validate_for_start(), [violation("2", 1)]);
    let source_snapshot = roster
        .entry_by_key(&source.0)
        .expect("the source team survives");
    assert_eq!(source_snapshot.plate, "2");
}

#[test]
fn test_validate_for_start_given_a_relay_team_reduced_to_one_reports_the_entry_plate() {
    let mut roster = relay();
    let team = relay_team(&mut roster, "Team A", "7");
    roster
        .remove_rider(&team, 1)
        .expect("the team keeps its remaining rider");

    assert_eq!(roster.validate_for_start(), [violation("7", 1)]);
}
