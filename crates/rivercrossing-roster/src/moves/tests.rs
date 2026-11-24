// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the rider moves, the extraction to solo, the addition to a team
//! and the rider removal.

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;

use crate::enums::{EntryMode, EntryType, PlateModel, RideStatus};
use crate::error::RosterError;
use crate::roster::{Roster, TeamFields};
use crate::types::{AuditEvent, Entry, EntryId, Rider};

fn pooled() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, None).expect("the settings are valid")
}

fn pooled_of_size(max_team_size: u32) -> Roster {
    Roster::new(
        EntryMode::Mixed,
        max_team_size,
        PlateModel::RiderPooled,
        None,
    )
    .expect("the settings are valid")
}

fn relay() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::TeamRelay, None).expect("the settings are valid")
}

fn seeded() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(8843))
        .expect("the settings are valid")
}

fn seeded_codes() -> Vec<String> {
    rivercrossing_cards::seeded_card_codes(8843, 1)
        .iter()
        .map(Card::code)
        .collect()
}

fn named(first_name: &str, plate: Option<&str>) -> Rider {
    Rider {
        first_name: first_name.to_owned(),
        plate: plate.map(str::to_owned),
        ..Rider::default()
    }
}

fn team(roster: &mut Roster, display_name: &str, members: &[(&str, &str)]) -> EntryId {
    let riders = members
        .iter()
        .map(|(name, plate)| named(name, Some(plate)))
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

fn team_of_one(roster: &mut Roster, display_name: &str) -> EntryId {
    roster
        .create_team_entry_of_one(TeamFields {
            display_name,
            riders: vec![named("Alex", Some("1"))],
            ..TeamFields::default()
        })
        .expect("a valid team of one is accepted")
}

fn solo(roster: &mut Roster, first_name: &str, plate: &str) -> EntryId {
    roster
        .create_solo_entry(named(first_name, None), plate)
        .expect("a fresh plate is accepted")
}

fn stored(roster: &Roster, id: &EntryId) -> Entry {
    roster
        .entry_by_key(&id.0)
        .expect("the roster still holds the entry")
        .clone()
}

fn log(action: &str, payload: serde_json::Value) -> AuditEvent {
    AuditEvent {
        action: action.to_owned(),
        payload,
    }
}

// ------------------------------------------------------------- move_rider

#[test]
fn test_move_rider_team_to_team_carries_the_pooled_plate_and_recomputes_both() {
    let mut roster = pooled();
    let team_a = team(
        &mut roster,
        "Team A",
        &[("Alex", "1"), ("Bo", "2"), ("El", "3")],
    );
    let team_b = team(&mut roster, "Team B", &[("Cy", "10"), ("Do", "11")]);

    roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect("the move is allowed");

    let source = stored(&roster, &team_a);
    let destination = stored(&roster, &team_b);
    assert_eq!(
        (
            source.plate.as_str(),
            destination.plate.as_str(),
            destination.riders.len()
        ),
        ("2", "1", 3)
    );
    assert_eq!(
        destination
            .riders
            .iter()
            .map(|rider| rider.plate.clone().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["10", "11", "1"]
    );
}

#[test]
fn test_move_rider_team_to_team_audits_both_plates() {
    let mut roster = pooled();
    team(
        &mut roster,
        "Team A",
        &[("Alex", "1"), ("Bo", "2"), ("El", "3")],
    );
    let team_b = team(&mut roster, "Team B", &[("Cy", "10"), ("Do", "11")]);
    roster.take_audit_log();

    roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect("the move is allowed");

    assert_eq!(
        roster.audit_log(),
        [log(
            "move_rider",
            serde_json::json!({"rider_name": "Alex", "from_plate": "2", "to_plate": "1"})
        )]
    );
}

#[test]
fn test_move_rider_team_to_team_leaves_a_size_one_source_team() {
    let mut roster = pooled();
    let team_a = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);

    roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect("a transient size-one team is allowed");

    assert_eq!(stored(&roster, &team_a).riders.len(), 1);
}

#[test]
fn test_move_rider_solo_to_team_moves_the_rider_and_dissolves_the_solo() {
    let mut roster = pooled();
    let solo = solo(&mut roster, "Alex", "9");
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);

    roster
        .move_rider("9", &team_b, "rider joined a team")
        .expect("the move is allowed");

    let destination = stored(&roster, &team_b);
    assert_eq!(
        (
            destination.plate.as_str(),
            destination.riders.len(),
            roster.entry_by_key(&solo.0).is_some()
        ),
        ("3", 3, false)
    );
}

#[test]
fn test_move_rider_solo_to_team_audits_the_move_then_the_dissolve() {
    let mut roster = pooled();
    solo(&mut roster, "Alex", "9");
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);
    roster.take_audit_log();

    roster
        .move_rider("9", &team_b, "rider joined a team")
        .expect("the move is allowed");

    assert_eq!(
        roster.audit_log(),
        [
            log(
                "move_rider",
                serde_json::json!({"rider_name": "Alex", "from_plate": "9", "to_plate": "3"})
            ),
            log(
                "dissolve_entry",
                serde_json::json!({"plate": "9", "display_name": "Alex"})
            ),
        ]
    );
}

#[test]
fn test_move_rider_of_a_size_one_teams_last_rider_dissolves_the_entry() {
    let mut roster = pooled();
    let team_a = team_of_one(&mut roster, "Team A");
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);

    roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect("the move is allowed");

    assert_eq!(roster.entry_by_key(&team_a.0).is_some(), false);
}

#[test]
fn test_move_rider_dissolve_audits_a_team_entry_with_its_plate_and_name() {
    let mut roster = pooled();
    team_of_one(&mut roster, "Team A");
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);
    roster.take_audit_log();

    roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect("the move is allowed");

    assert_eq!(
        roster.audit_log().last(),
        Some(&log(
            "dissolve_team_entry",
            serde_json::json!({"plate": "1", "display_name": "Team A"})
        ))
    );
}

#[test]
fn test_move_rider_into_a_size_one_team_grows_it_to_two() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let team_c = roster
        .create_team_entry_of_one(TeamFields {
            display_name: "Team C",
            riders: vec![named("Cy", Some("9"))],
            ..TeamFields::default()
        })
        .expect("a valid team of one is accepted");

    roster
        .move_rider("1", &team_c, "rider swapped teams")
        .expect("the move is allowed");

    assert_eq!(stored(&roster, &team_c).riders.len(), 2);
}

#[test]
fn test_move_rider_filling_the_destination_to_max_succeeds() {
    let mut roster = pooled_of_size(4);
    team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let team_b = team(
        &mut roster,
        "Team B",
        &[("Cy", "3"), ("Do", "4"), ("El", "5")],
    );

    roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect("a destination landing exactly on max is admitted");

    assert_eq!(stored(&roster, &team_b).riders.len(), 4);
}

#[test]
fn test_move_rider_over_the_destination_max_refuses_verbatim() {
    let mut roster = pooled_of_size(4);
    let team_a = team(
        &mut roster,
        "Team A",
        &[("Alex", "1"), ("Bo", "2"), ("El", "3")],
    );
    let team_b = team(
        &mut roster,
        "Team B",
        &[("Cy", "4"), ("Do", "5"), ("Fy", "6"), ("Gy", "7")],
    );

    let error = roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect_err("the destination is full");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "move would exceed the destination team's max size".to_owned(),
        }
    );
    assert_eq!(stored(&roster, &team_a).riders.len(), 3);
}

#[test]
fn test_move_rider_into_a_solo_entry_refuses() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let solo = solo(&mut roster, "Cy", "3");

    let error = roster
        .move_rider("1", &solo, "rider swapped teams")
        .expect_err("a solo destination is refused");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "move_rider's destination must be a team entry".to_owned(),
        }
    );
}

#[test]
fn test_move_rider_given_an_unknown_rider_plate_refuses() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);

    let error = roster
        .move_rider("999", &team_b, "rider swapped teams")
        .expect_err("the rider is on no entry");

    assert_eq!(
        error,
        RosterError::RiderNotFound {
            description: "rider is not on any entry in this roster".to_owned(),
        }
    );
}

#[test]
fn test_move_rider_given_an_unknown_destination_refuses() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let ghost = EntryId("ghost".to_owned());

    let error = roster
        .move_rider("1", &ghost, "rider swapped teams")
        .expect_err("a foreign destination is refused");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
}

#[test]
fn test_move_rider_into_a_retired_team_is_accepted() {
    let mut roster = pooled();
    let team_a = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let retired = Entry::restored(
        "retired1",
        "7",
        "Old Team",
        EntryType::Team,
        vec![named("Cy", Some("20")), named("Do", Some("21"))],
    );
    roster.load_retired_entries(vec![retired]);
    roster.take_audit_log();

    roster
        .move_rider("1", &EntryId("retired1".to_owned()), "late correction")
        .expect("a retired team is a known destination");

    let destination = roster
        .entry_by_key("retired1")
        .expect("the retired team stays known");
    assert_eq!(
        (
            stored(&roster, &team_a).plate.as_str(),
            destination.plate.as_str(),
            destination.riders.len()
        ),
        ("2", "1", 3)
    );
    assert_eq!(
        roster.audit_log(),
        [log(
            "move_rider",
            serde_json::json!({"rider_name": "Alex", "from_plate": "2", "to_plate": "1"})
        )]
    );
}

#[test]
fn test_move_rider_on_a_relay_ride_keeps_each_entrys_own_plate() {
    let mut roster = relay();
    roster.load_entries(vec![
        Entry::restored(
            "a",
            "5",
            "Team A",
            EntryType::Team,
            vec![named("Alex", Some("1")), named("Bo", Some("2"))],
        ),
        Entry::restored(
            "b",
            "6",
            "Team B",
            EntryType::Team,
            vec![named("Cy", Some("3")), named("Do", Some("4"))],
        ),
    ]);

    roster
        .move_rider("1", &EntryId("b".to_owned()), "rider swapped teams")
        .expect("a draft relay ride allows the move");

    assert_eq!(
        (
            roster
                .entry_by_key("a")
                .expect("Team A exists")
                .plate
                .as_str(),
            roster
                .entry_by_key("b")
                .expect("Team B exists")
                .plate
                .as_str(),
            roster
                .entry_by_key("b")
                .expect("Team B exists")
                .riders
                .len()
        ),
        ("5", "6", 3)
    );
}

#[test]
fn test_move_rider_on_a_relay_ride_after_start_refuses_locked() {
    let mut roster = relay();
    let team_a = relay_team(&mut roster, "Team A", "1");
    let team_b = relay_team(&mut roster, "Team B", "2");
    roster.set_status(RideStatus::Running);

    let error = roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect_err("a relay ride keeps the start lock");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "rider moves are locked for a team_relay ride once running".to_owned(),
        }
    );
    assert_eq!(stored(&roster, &team_a).riders.len(), 2);
}

#[test]
fn test_move_rider_on_a_finished_pooled_ride_refuses_locked() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);
    roster.set_status(RideStatus::Finished);

    let error = roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect_err("a finished ride closes the pooled door");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "rider moves are locked for a rider_pooled ride once finished".to_owned(),
        }
    );
}

#[test]
fn test_move_rider_on_a_running_pooled_ride_succeeds() {
    let mut roster = pooled();
    solo(&mut roster, "Alex", "1");
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);
    roster.set_status(RideStatus::Running);

    roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect("a running pooled ride stays open");

    assert_eq!(stored(&roster, &team_b).riders.len(), 3);
}

#[test]
fn test_move_rider_on_a_reopened_pooled_ride_succeeds() {
    let mut roster = pooled();
    solo(&mut roster, "Alex", "1");
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);
    roster.set_status(RideStatus::Reopened);

    roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect("a reopened pooled ride allows the correction");

    assert_eq!(stored(&roster, &team_b).riders.len(), 3);
}

#[test]
fn test_move_rider_dissolving_a_data_bearing_solo_retires_it() {
    let mut roster = pooled();
    let solo_id = solo(&mut roster, "Alex", "1");
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);
    roster
        .mark_has_data(&solo_id)
        .expect("a live entry accepts the flag");

    roster
        .move_rider("1", &team_b, "rider joined a team")
        .expect("a data-bearing source still moves");

    assert_eq!(roster.retired_entries().len(), 1);
    assert_eq!(roster.retired_entries()[0].key, solo_id.0);
    assert_eq!(roster.entry_by_key(&solo_id.0).is_some(), true);
}

#[test]
fn test_move_rider_dissolving_a_data_less_solo_drops_it() {
    let mut roster = pooled();
    let solo_id = solo(&mut roster, "Alex", "1");
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);

    roster
        .move_rider("1", &team_b, "rider joined a team")
        .expect("the move is allowed");

    assert_eq!(roster.retired_entries(), []);
    assert_eq!(roster.entry_by_key(&solo_id.0).is_some(), false);
}

#[test]
fn test_move_rider_dissolving_a_data_bearing_team_returns_its_logo_card_to_the_pool() {
    let cards = seeded_codes();
    let mut roster = seeded();
    let team_a = roster
        .create_team_entry_of_one(TeamFields {
            display_name: "Team A",
            riders: vec![named("Alex", Some("1"))],
            ..TeamFields::default()
        })
        .expect("a valid team of one is accepted");
    let team_b = roster
        .create_team_entry_of_one(TeamFields {
            display_name: "Team B",
            riders: vec![named("Cy", Some("3"))],
            ..TeamFields::default()
        })
        .expect("a valid team of one is accepted");
    roster
        .mark_has_data(&team_a)
        .expect("a live entry accepts the flag");

    roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect("the move is allowed");

    assert_eq!(roster.next_team_logo_card(None), Some(cards[0].clone()));
}

// -------------------------------------------------- extract_rider_to_solo

#[test]
fn test_extract_rider_to_solo_creates_a_solo_entry_with_the_riders_plate_and_name() {
    let mut roster = pooled();
    let team = team(
        &mut roster,
        "Team A",
        &[("Alex", "5"), ("Bo", "9"), ("Cy", "1")],
    );

    let solo_id = roster
        .extract_rider_to_solo("5", "rider went solo")
        .expect("a pooled team member is extracted");

    let entry = stored(&roster, &solo_id);
    assert_eq!(
        (
            entry.kind,
            entry.plate.as_str(),
            entry.display_name.as_str(),
            entry.riders.len(),
            stored(&roster, &team).plate.as_str()
        ),
        (EntryType::Solo, "5", "Alex", 1, "1")
    );
}

#[test]
fn test_extract_rider_to_solo_returns_a_new_key_distinct_from_the_source() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    let solo_id = roster
        .extract_rider_to_solo("1", "rider went solo")
        .expect("a pooled team member is extracted");

    assert_eq!((solo_id.0 == team.0, solo_id.0.len()), (false, 32));
}

#[test]
fn test_extract_rider_to_solo_recomputes_the_source_teams_plate() {
    let mut roster = pooled();
    let team = team(
        &mut roster,
        "Team A",
        &[("Alex", "1"), ("Bo", "9"), ("Cy", "5")],
    );

    roster
        .extract_rider_to_solo("1", "rider went solo")
        .expect("a pooled team member is extracted");

    assert_eq!(stored(&roster, &team).plate, "5");
}

#[test]
fn test_extract_rider_to_solo_audits_the_rider_and_the_source() {
    let mut roster = pooled();
    team(
        &mut roster,
        "Team A",
        &[("Alex", "5"), ("Bo", "9"), ("Cy", "1")],
    );
    roster.take_audit_log();

    roster
        .extract_rider_to_solo("5", "rider went solo")
        .expect("a pooled team member is extracted");

    assert_eq!(
        roster.audit_log(),
        [log(
            "extract_rider_to_solo",
            serde_json::json!({"rider_name": "Alex", "plate": "5", "from_display_name": "Team A"})
        )]
    );
}

#[test]
fn test_extract_rider_to_solo_of_the_last_member_dissolves_the_source() {
    let mut roster = pooled();
    let team_a = team_of_one(&mut roster, "Team A");
    roster.take_audit_log();

    roster
        .extract_rider_to_solo("1", "rider went solo")
        .expect("a pooled team member is extracted");

    assert_eq!(
        (
            roster.entry_by_key(&team_a.0).is_some(),
            roster.audit_log().last()
        ),
        (
            false,
            Some(&log(
                "dissolve_team_entry",
                serde_json::json!({"plate": "1", "display_name": "Team A"})
            ))
        )
    );
}

#[test]
fn test_extract_rider_to_solo_of_the_last_member_keeps_the_solo_entry() {
    let mut roster = pooled();
    team_of_one(&mut roster, "Team A");

    let solo_id = roster
        .extract_rider_to_solo("1", "rider went solo")
        .expect("a pooled team member is extracted");

    assert_eq!(
        (
            roster.entries().len(),
            roster.entries()[0].key.as_str(),
            roster.entries()[0].kind
        ),
        (1, solo_id.0.as_str(), EntryType::Solo)
    );
}

#[test]
fn test_extract_rider_to_solo_on_a_running_pooled_ride_succeeds() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);
    roster.set_status(RideStatus::Running);

    let solo_id = roster
        .extract_rider_to_solo("5", "rider went solo")
        .expect("a running pooled ride stays open");

    assert_eq!(stored(&roster, &solo_id).plate, "5");
}

#[test]
fn test_extract_rider_to_solo_on_a_reopened_pooled_ride_succeeds() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "2")]);
    roster.set_status(RideStatus::Reopened);

    roster
        .extract_rider_to_solo("5", "rider went solo")
        .expect("a reopened pooled ride allows the correction");

    assert_eq!(roster.entries()[0].plate, "2");
}

#[test]
fn test_extract_rider_to_solo_on_a_running_relay_ride_refuses_locked() {
    let mut roster = relay();
    relay_team(&mut roster, "Team A", "1");
    roster.set_status(RideStatus::Running);

    let error = roster
        .extract_rider_to_solo("1", "rider went solo")
        .expect_err("a relay ride keeps the start lock");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "a rider cannot be extracted to solo on a team_relay ride once running"
                .to_owned(),
        }
    );
}

#[test]
fn test_extract_rider_to_solo_on_a_finished_pooled_ride_refuses_locked() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);
    roster.set_status(RideStatus::Finished);

    let error = roster
        .extract_rider_to_solo("5", "rider went solo")
        .expect_err("a finished ride closes the pooled door");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "a rider cannot be extracted to solo on a rider_pooled ride once finished"
                .to_owned(),
        }
    );
}

#[test]
fn test_extract_rider_to_solo_on_a_relay_team_refuses() {
    let mut roster = relay();
    roster.load_entries(vec![Entry::restored(
        "a",
        "1",
        "Team A",
        EntryType::Team,
        vec![named("Alex", Some("5")), named("Bo", None)],
    )]);

    let error = roster
        .extract_rider_to_solo("5", "rider went solo")
        .expect_err("a relay rider has no plate identity to extract");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "extract_rider_to_solo requires a rider_pooled team member".to_owned(),
        }
    );
}

#[test]
fn test_extract_rider_to_solo_given_an_already_solo_rider_refuses() {
    let mut roster = pooled();
    solo(&mut roster, "Alex", "1");

    let error = roster
        .extract_rider_to_solo("1", "rider went solo")
        .expect_err("a solo rider has nothing to extract from");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "extract_rider_to_solo requires a rider_pooled team member".to_owned(),
        }
    );
}

#[test]
fn test_extract_rider_to_solo_given_an_unknown_rider_plate_refuses() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);

    let error = roster
        .extract_rider_to_solo("999", "rider went solo")
        .expect_err("the rider is on no entry");

    assert_eq!(
        error,
        RosterError::RiderNotFound {
            description: "rider is not on any entry in this roster".to_owned(),
        }
    );
}

#[test]
fn test_extract_rider_to_solo_dissolving_a_data_bearing_team_retires_it() {
    let mut roster = pooled();
    let team_a = team_of_one(&mut roster, "Team A");
    roster
        .mark_has_data(&team_a)
        .expect("a live entry accepts the flag");

    roster
        .extract_rider_to_solo("1", "rider went solo")
        .expect("a data-bearing source still extracts");

    assert_eq!(
        (
            roster.retired_entries().len(),
            roster.retired_entries()[0].key.as_str(),
            roster.entry_by_key(&team_a.0).is_some()
        ),
        (1, team_a.0.as_str(), true)
    );
}

// ------------------------------------------------------- add_rider_to_team

#[test]
fn test_add_rider_to_team_pooled_appends_and_recomputes_the_plate() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);

    roster
        .add_rider_to_team(named("Cy", Some("1")), &team)
        .expect("the add is allowed");

    let entry = stored(&roster, &team);
    assert_eq!(
        (
            entry.plate.as_str(),
            entry.riders.len(),
            entry.riders[2].plate.clone().unwrap_or_default()
        ),
        ("1", 3, "1".to_owned())
    );
}

#[test]
fn test_add_rider_to_team_relay_clears_the_riders_plate() {
    let mut roster = relay();
    let team = relay_team(&mut roster, "Team A", "1");

    roster
        .add_rider_to_team(named("Cy", Some("9")), &team)
        .expect("the add is allowed");

    let entry = stored(&roster, &team);
    assert_eq!(
        (entry.plate.as_str(), entry.riders[2].plate.clone()),
        ("1", None)
    );
}

#[test]
fn test_add_rider_to_team_pooled_audits_the_rider_and_the_team() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);
    roster.take_audit_log();

    roster
        .add_rider_to_team(named("Cy", Some("1")), &team)
        .expect("the add is allowed");

    assert_eq!(
        roster.audit_log(),
        [log(
            "add_rider_to_team",
            serde_json::json!({"rider_name": "Cy", "rider_plate": "1", "to_plate": "1", "display_name": "Team A"})
        )]
    );
}

#[test]
fn test_add_rider_to_team_relay_audits_a_null_rider_plate() {
    let mut roster = relay();
    let team = relay_team(&mut roster, "Team A", "1");
    roster.take_audit_log();

    roster
        .add_rider_to_team(named("Cy", Some("9")), &team)
        .expect("the add is allowed");

    assert_eq!(
        roster.audit_log(),
        [log(
            "add_rider_to_team",
            serde_json::json!({"rider_name": "Cy", "rider_plate": null, "to_plate": "1", "display_name": "Team A"})
        )]
    );
}

#[test]
fn test_add_rider_to_team_pooled_on_a_running_ride_succeeds() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);
    roster.set_status(RideStatus::Running);

    roster
        .add_rider_to_team(named("Cy", Some("1")), &team)
        .expect("a running pooled ride stays open");

    assert_eq!(stored(&roster, &team).riders.len(), 3);
}

#[test]
fn test_add_rider_to_team_pooled_on_a_reopened_ride_succeeds() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);
    roster.set_status(RideStatus::Reopened);

    roster
        .add_rider_to_team(named("Cy", Some("1")), &team)
        .expect("a reopened pooled ride allows the correction");

    assert_eq!(stored(&roster, &team).riders.len(), 3);
}

#[test]
fn test_add_rider_to_team_on_a_relay_ride_after_start_refuses_locked() {
    let mut roster = relay();
    let team = relay_team(&mut roster, "Team A", "1");
    roster.set_status(RideStatus::Running);

    let error = roster
        .add_rider_to_team(named("Cy", None), &team)
        .expect_err("a relay ride keeps the start lock");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "rider moves are locked for a team_relay ride once running".to_owned(),
        }
    );
}

#[test]
fn test_add_rider_to_team_on_a_finished_pooled_ride_refuses_locked() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);
    roster.set_status(RideStatus::Finished);

    let error = roster
        .add_rider_to_team(named("Cy", Some("1")), &team)
        .expect_err("a finished ride closes the pooled door");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "rider moves are locked for a rider_pooled ride once finished".to_owned(),
        }
    );
}

#[test]
fn test_add_rider_to_team_on_a_solo_entry_refuses() {
    let mut roster = pooled();
    let solo = solo(&mut roster, "Alex", "1");

    let error = roster
        .add_rider_to_team(named("Cy", Some("2")), &solo)
        .expect_err("a solo entry takes no second rider");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "add_rider_to_team requires a team entry".to_owned(),
        }
    );
}

#[test]
fn test_add_rider_to_team_at_max_size_refuses_verbatim() {
    let mut roster = pooled_of_size(2);
    let team = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    let error = roster
        .add_rider_to_team(named("Cy", Some("3")), &team)
        .expect_err("the team is full");

    assert_eq!(
        error,
        RosterError::TeamSize {
            message: "team size must be at most 2, got 3".to_owned(),
        }
    );
}

#[test]
fn test_add_rider_to_team_given_an_already_placed_rider_refuses() {
    let mut roster = pooled();
    let team_a = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);
    let placed = stored(&roster, &team_a).riders[0].clone();

    let error = roster
        .add_rider_to_team(placed, &team_b)
        .expect_err("a placed rider must move instead");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "rider is already on an entry; use move_rider instead".to_owned(),
        }
    );
}

#[test]
fn test_add_rider_to_team_given_a_duplicate_plate_refuses() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    let error = roster
        .add_rider_to_team(named("Cy", Some("1")), &team)
        .expect_err("the plate is claimed");

    assert_eq!(
        error,
        RosterError::DuplicatePlate {
            plate: "1".to_owned(),
        }
    );
}

#[test]
fn test_add_rider_to_team_pooled_given_a_riderless_plate_refuses() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    let error = roster
        .add_rider_to_team(named("Cy", None), &team)
        .expect_err("a pooled rider must carry a plate");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "rider_pooled riders must each carry a plate".to_owned(),
        }
    );
}

#[test]
fn test_add_rider_to_team_given_a_non_numeric_plate_refuses() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    let error = roster
        .add_rider_to_team(named("Cy", Some("7A")), &team)
        .expect_err("a letter plate is refused");

    assert_eq!(
        error,
        RosterError::PlateShape {
            plate: "7A".to_owned(),
            reason: "must be a whole number".to_owned(),
        }
    );
}

#[test]
fn test_add_rider_to_team_given_an_unknown_team_refuses() {
    let mut roster = pooled();

    let error = roster
        .add_rider_to_team(named("Cy", Some("1")), &EntryId("ghost".to_owned()))
        .expect_err("a foreign team is refused");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
}

#[test]
fn test_add_rider_to_team_to_a_retired_team_is_accepted() {
    let mut roster = pooled();
    let retired = Entry::restored(
        "retired1",
        "7",
        "Old Team",
        EntryType::Team,
        vec![named("Cy", Some("20")), named("Do", Some("21"))],
    );
    roster.load_retired_entries(vec![retired]);
    roster.take_audit_log();

    roster
        .add_rider_to_team(named("El", Some("30")), &EntryId("retired1".to_owned()))
        .expect("a retired team is a known destination");

    let entry = roster
        .entry_by_key("retired1")
        .expect("the retired team stays known");
    assert_eq!((entry.plate.as_str(), entry.riders.len()), ("20", 3));
    assert_eq!(
        roster.audit_log(),
        [log(
            "add_rider_to_team",
            serde_json::json!({"rider_name": "El", "rider_plate": "30", "to_plate": "20", "display_name": "Old Team"})
        )]
    );
}

// ----------------------------------------------------------- remove_rider

#[test]
fn test_remove_rider_solo_delegates_to_delete_entry() {
    let mut roster = pooled();
    let solo = solo(&mut roster, "Alex", "1");
    roster.take_audit_log();

    roster
        .remove_rider(&solo, 0)
        .expect("a solo removal deletes the entry");

    assert_eq!(
        (roster.entries(), roster.audit_log()),
        (
            &[][..],
            &[log(
                "delete_entry",
                serde_json::json!({"plate": "1", "display_name": "Alex"})
            )][..]
        )
    );
}

#[test]
fn test_remove_rider_solo_after_start_refuses_locked() {
    let mut roster = pooled();
    let solo = solo(&mut roster, "Alex", "1");
    roster.set_status(RideStatus::Running);

    let error = roster
        .remove_rider(&solo, 0)
        .expect_err("a post-start delete is refused");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "entries can no longer be deleted once the ride is running".to_owned(),
        }
    );
}

#[test]
fn test_remove_rider_team_member_removes_only_that_rider() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);

    roster
        .remove_rider(&team, 0)
        .expect("a member removal leaves the team");

    let entry = stored(&roster, &team);
    assert_eq!(
        (
            roster.entries().len(),
            entry
                .riders
                .iter()
                .map(Rider::full_name)
                .collect::<Vec<_>>()
        ),
        (1, vec!["Bo".to_owned()])
    );
}

#[test]
fn test_remove_rider_from_a_three_rider_team_keeps_the_other_two() {
    let mut roster = pooled();
    let team = team(
        &mut roster,
        "Team A",
        &[("Alex", "5"), ("Bo", "9"), ("Cy", "12")],
    );

    roster
        .remove_rider(&team, 1)
        .expect("a member removal leaves the team");

    assert_eq!(
        stored(&roster, &team)
            .riders
            .iter()
            .map(Rider::full_name)
            .collect::<Vec<_>>(),
        vec!["Alex".to_owned(), "Cy".to_owned()]
    );
}

#[test]
fn test_remove_rider_of_the_lowest_plate_member_recomputes_the_plate() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "9")]);

    roster
        .remove_rider(&team, 0)
        .expect("a member removal leaves the team");

    assert_eq!(stored(&roster, &team).plate, "9");
}

#[test]
fn test_remove_rider_audits_the_removed_rider_and_the_entry() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);
    roster.take_audit_log();

    roster
        .remove_rider(&team, 1)
        .expect("a member removal leaves the team");

    assert_eq!(
        roster.audit_log(),
        [log(
            "remove_rider",
            serde_json::json!({"rider_name": "Bo", "plate": "5", "display_name": "Team A"})
        )]
    );
}

#[test]
fn test_remove_rider_from_a_relay_team_keeps_the_entry_plate() {
    let mut roster = relay();
    let team = relay_team(&mut roster, "Team A", "7");

    roster
        .remove_rider(&team, 0)
        .expect("a member removal leaves the team");

    let entry = stored(&roster, &team);
    assert_eq!(
        (
            entry.plate.as_str(),
            entry
                .riders
                .iter()
                .map(Rider::full_name)
                .collect::<Vec<_>>()
        ),
        ("7", vec!["Bo".to_owned()])
    );
}

#[test]
fn test_remove_rider_of_a_relay_rider_by_index_needs_no_plate() {
    let mut roster = relay();
    let team = relay_team(&mut roster, "Team A", "7");

    roster
        .remove_rider(&team, 1)
        .expect("an unplated relay rider is addressed by its index");

    assert_eq!(stored(&roster, &team).riders.len(), 1);
}

#[test]
fn test_remove_rider_of_the_last_member_dissolves_the_team() {
    let mut roster = pooled();
    let team = team_of_one(&mut roster, "Team A");

    roster
        .remove_rider(&team, 0)
        .expect("the last member is removed");

    assert_eq!(roster.entry_by_key(&team.0).is_some(), false);
}

#[test]
fn test_remove_rider_of_the_last_member_audits_the_dissolve_then_the_removal() {
    let mut roster = pooled();
    let team = team_of_one(&mut roster, "Team A");
    roster.take_audit_log();

    roster
        .remove_rider(&team, 0)
        .expect("the last member is removed");

    assert_eq!(
        roster
            .audit_log()
            .iter()
            .map(|event| event.action.as_str())
            .collect::<Vec<_>>(),
        ["dissolve_team_entry", "remove_rider"]
    );
}

#[test]
fn test_remove_rider_on_a_data_bearing_team_refuses_locked_verbatim() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);
    roster
        .mark_has_data(&team)
        .expect("a live entry accepts the flag");

    let error = roster
        .remove_rider(&team, 0)
        .expect_err("a data-bearing team keeps its members");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "entry has recorded data; DNF or void it instead of deleting".to_owned(),
        }
    );
}

#[test]
fn test_remove_rider_from_a_team_after_start_refuses_locked() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);
    roster.set_status(RideStatus::Running);

    let error = roster
        .remove_rider(&team, 0)
        .expect_err("a removal is a draft-only structural edit");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "entries can no longer be deleted once the ride is running".to_owned(),
        }
    );
}

#[test]
fn test_remove_rider_given_an_unknown_entry_refuses() {
    let mut roster = pooled();

    let error = roster
        .remove_rider(&EntryId("ghost".to_owned()), 0)
        .expect_err("a foreign entry is refused");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
}

#[test]
fn test_remove_rider_given_an_index_out_of_range_refuses() {
    let mut roster = pooled();
    let team = team(&mut roster, "Team A", &[("Alex", "5"), ("Bo", "9")]);

    let error = roster
        .remove_rider(&team, 2)
        .expect_err("the team has no third rider");

    assert_eq!(
        error,
        RosterError::RiderNotFound {
            description: "rider is not on any entry in this roster".to_owned(),
        }
    );
}

#[test]
fn test_remove_rider_frees_the_plate_for_a_new_entry() {
    let mut roster = pooled();
    let team = team_of_one(&mut roster, "Team A");
    roster
        .remove_rider(&team, 0)
        .expect("the last member is removed");

    let solo = solo(&mut roster, "Cy", "1");

    assert_eq!(stored(&roster, &solo).plate, "1");
}

// ---------------------------------------------------------- move_rider_at

#[test]
fn test_move_rider_at_moves_a_plateless_relay_rider_in_draft() {
    let mut roster = relay();
    let team_a = relay_team(&mut roster, "Team A", "1");
    let team_b = relay_team(&mut roster, "Team B", "2");

    roster
        .move_rider_at(&team_a, 1, &team_b, "relay swap")
        .expect("a draft relay ride allows the move");

    let source = stored(&roster, &team_a);
    let destination = stored(&roster, &team_b);
    assert_eq!(
        (
            source.riders.len(),
            source.plate.as_str(),
            destination.riders.len(),
            destination.riders[2].plate.clone(),
            destination.plate.as_str(),
        ),
        (1, "1", 3, None, "2")
    );
}

#[test]
fn test_move_rider_at_after_start_on_a_relay_ride_refuses_locked() {
    let mut roster = relay();
    let team_a = relay_team(&mut roster, "Team A", "1");
    let team_b = relay_team(&mut roster, "Team B", "2");
    roster.set_status(RideStatus::Running);

    let error = roster
        .move_rider_at(&team_a, 0, &team_b, "relay swap")
        .expect_err("a relay ride keeps the start lock");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "rider moves are locked for a team_relay ride once running".to_owned(),
        }
    );
    assert_eq!(stored(&roster, &team_a).riders.len(), 2);
}

#[test]
fn test_move_rider_at_pooled_recomputes_both_teams() {
    let mut roster = pooled();
    let team_a = team(
        &mut roster,
        "Team A",
        &[("Alex", "1"), ("Bo", "2"), ("El", "3")],
    );
    let team_b = team(&mut roster, "Team B", &[("Cy", "10"), ("Do", "11")]);

    roster
        .move_rider_at(&team_a, 0, &team_b, "rider swapped teams")
        .expect("the move is allowed");

    assert_eq!(
        (
            stored(&roster, &team_a).plate.as_str(),
            stored(&roster, &team_b).plate.as_str(),
        ),
        ("2", "1")
    );
}

#[test]
fn test_move_rider_at_of_the_last_member_dissolves_the_source_team() {
    let mut roster = pooled();
    let team_a = team_of_one(&mut roster, "Team A");
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);
    roster.take_audit_log();

    roster
        .move_rider_at(&team_a, 0, &team_b, "rider swapped teams")
        .expect("the move is allowed");

    assert_eq!(
        (
            roster.entry_by_key(&team_a.0).is_some(),
            roster.audit_log().last().map(|event| event.action.as_str()),
        ),
        (false, Some("dissolve_team_entry"))
    );
}

#[test]
fn test_move_rider_at_appends_one_move_rider_event() {
    let mut roster = pooled();
    let team_a = team(
        &mut roster,
        "Team A",
        &[("Alex", "1"), ("Bo", "2"), ("El", "3")],
    );
    let team_b = team(&mut roster, "Team B", &[("Cy", "10"), ("Do", "11")]);
    roster.take_audit_log();

    roster
        .move_rider_at(&team_a, 0, &team_b, "rider swapped teams")
        .expect("the move is allowed");

    assert_eq!(
        roster.audit_log(),
        [log(
            "move_rider",
            serde_json::json!({"rider_name": "Alex", "from_plate": "2", "to_plate": "1"})
        )]
    );
}

#[test]
fn test_move_rider_at_given_an_unknown_destination_refuses() {
    let mut roster = pooled();
    let team_a = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    let error = roster
        .move_rider_at(
            &team_a,
            0,
            &EntryId("ghost".to_owned()),
            "rider swapped teams",
        )
        .expect_err("a foreign destination is refused");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
    assert_eq!(stored(&roster, &team_a).riders.len(), 2);
}

#[test]
fn test_move_rider_at_given_an_unknown_source_refuses() {
    let mut roster = pooled();
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);

    let error = roster
        .move_rider_at(
            &EntryId("ghost".to_owned()),
            0,
            &team_b,
            "rider swapped teams",
        )
        .expect_err("a foreign source is refused");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
}

#[test]
fn test_move_rider_at_given_an_index_out_of_range_refuses() {
    let mut roster = pooled();
    let team_a = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);
    roster.take_audit_log();

    let error = roster
        .move_rider_at(&team_a, 2, &team_b, "rider swapped teams")
        .expect_err("the team has no third rider");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: format!("entry {} has no rider at index 2", team_a.0),
        }
    );
    assert_eq!(
        (
            stored(&roster, &team_a).riders.len(),
            roster.audit_log().len()
        ),
        (2, 0)
    );
}

#[test]
fn test_move_rider_at_given_the_same_entry_succeeds() {
    let mut roster = pooled();
    let team_a = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    roster.take_audit_log();

    roster
        .move_rider_at(&team_a, 0, &team_a, "rider swapped teams")
        .expect("a same-entry move is a take and re-place");

    let entry = stored(&roster, &team_a);
    assert_eq!(
        (
            entry.riders.len(),
            entry
                .riders
                .iter()
                .map(Rider::full_name)
                .collect::<Vec<_>>(),
            entry.plate.as_str(),
        ),
        (2, vec!["Bo".to_owned(), "Alex".to_owned()], "1")
    );
    assert_eq!(
        roster.audit_log(),
        [log(
            "move_rider",
            serde_json::json!({"rider_name": "Alex", "from_plate": "1", "to_plate": "1"})
        )]
    );
}

#[test]
fn test_move_rider_given_the_same_entry_succeeds() {
    let mut roster = pooled();
    let team_a = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    roster.take_audit_log();

    roster
        .move_rider("1", &team_a, "rider swapped teams")
        .expect("a same-entry move is a take and re-place");

    let entry = stored(&roster, &team_a);
    assert_eq!(
        (
            entry.riders.len(),
            entry
                .riders
                .iter()
                .map(Rider::full_name)
                .collect::<Vec<_>>(),
            roster
                .audit_log()
                .first()
                .map(|event| event.action.as_str()),
        ),
        (
            2,
            vec!["Bo".to_owned(), "Alex".to_owned()],
            Some("move_rider")
        )
    );
}

#[test]
fn test_move_rider_still_resolves_by_plate() {
    let mut roster = pooled();
    let team_a = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let team_b = team(&mut roster, "Team B", &[("Cy", "3"), ("Do", "4")]);

    roster
        .move_rider("1", &team_b, "rider swapped teams")
        .expect("the plate path still resolves the rider");

    assert_eq!(
        (
            stored(&roster, &team_a).plate.as_str(),
            stored(&roster, &team_b).riders.len(),
        ),
        ("2", 3)
    );
}
