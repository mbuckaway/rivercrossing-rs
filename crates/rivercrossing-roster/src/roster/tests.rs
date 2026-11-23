// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the roster aggregate's constructor and read accessors.

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;
use rstest::rstest;

use super::Roster;
use crate::enums::{EntryMode, EntryStatus, EntryType, PlateModel, RideStatus};
use crate::error::RosterError;
use crate::types::{AuditEvent, Entry};

fn roster() -> Roster {
    Roster::new(EntryMode::Solo, 4, PlateModel::RiderPooled, None)
        .expect("the default roster settings are valid")
}

fn event(action: &str, count: i64) -> AuditEvent {
    AuditEvent {
        action: action.to_owned(),
        payload: serde_json::json!({"count": count}),
    }
}

fn entry(key: &str, plate: &str) -> Entry {
    Entry::restored(key, plate, "Sam Ellis", EntryType::Solo, Vec::new())
}

// --------------------------------------------------------- team-size bound

#[rstest]
#[case(1)]
#[case(11)]
fn test_roster_new_given_max_team_size_out_of_range_is_refused(#[case] size: u32) {
    let error = Roster::new(EntryMode::Solo, size, PlateModel::RiderPooled, None)
        .expect_err("a size outside 2..10 is refused");

    assert_eq!(
        error,
        RosterError::TeamSize {
            message: format!("max_team_size must be between 2 and 10, got {size}"),
        }
    );
}

#[test]
fn test_roster_new_given_the_min_team_size_accepts_two() {
    let roster = Roster::new(EntryMode::Solo, 2, PlateModel::RiderPooled, None)
        .expect("two is the smallest valid team");

    assert_eq!(roster.max_team_size(), 2);
}

#[test]
fn test_roster_new_given_min_plus_one_team_size_accepts_three() {
    let roster = Roster::new(EntryMode::Solo, 3, PlateModel::RiderPooled, None)
        .expect("three is a valid team size");

    assert_eq!(roster.max_team_size(), 3);
}

#[test]
fn test_roster_new_given_the_max_team_size_accepts_ten() {
    let roster = Roster::new(EntryMode::Solo, 10, PlateModel::RiderPooled, None)
        .expect("ten is the largest valid team");

    assert_eq!(roster.max_team_size(), 10);
}

#[test]
fn test_roster_new_given_max_minus_one_team_size_accepts_nine() {
    let roster = Roster::new(EntryMode::Solo, 9, PlateModel::RiderPooled, None)
        .expect("nine is a valid team size");

    assert_eq!(roster.max_team_size(), 9);
}

// ----------------------------------------------------------- initial state

#[test]
fn test_roster_new_given_no_entries_returns_an_empty_entry_list() {
    assert_eq!(roster().entries(), []);
}

#[test]
fn test_roster_new_given_no_entries_returns_an_empty_retired_list() {
    assert_eq!(roster().retired_entries(), []);
}

#[test]
fn test_roster_new_given_no_events_returns_an_empty_audit_log() {
    assert_eq!(roster().audit_log(), []);
}

#[test]
fn test_roster_new_starts_in_draft() {
    assert_eq!(roster().status(), RideStatus::Draft);
}

#[test]
fn test_roster_new_keeps_the_given_entry_mode() {
    let roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, None)
        .expect("the settings are valid");

    assert_eq!(roster.entry_mode(), EntryMode::Mixed);
}

#[test]
fn test_roster_new_keeps_the_given_plate_model() {
    let roster = Roster::new(EntryMode::Mixed, 4, PlateModel::TeamRelay, None)
        .expect("the settings are valid");

    assert_eq!(roster.plate_model(), PlateModel::TeamRelay);
}

// -------------------------------------------------------------- logo deck

#[test]
fn test_roster_new_given_a_seed_keeps_the_seed() {
    let roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(8843))
        .expect("the settings are valid");

    assert_eq!(roster.team_logo_seed(), Some(8843));
}

#[test]
fn test_roster_new_given_no_seed_reports_no_seed() {
    assert_eq!(roster().team_logo_seed(), None);
}

#[test]
fn test_roster_new_given_a_seed_builds_the_seeded_card_deck() {
    let roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(8843))
        .expect("the settings are valid");

    let expected: Vec<String> = rivercrossing_cards::seeded_card_codes(8843, 1)
        .iter()
        .map(Card::code)
        .collect();
    assert_eq!(roster.logo_codes, expected);
}

#[test]
fn test_roster_new_given_a_seed_builds_fifty_two_logo_codes() {
    let roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(8843))
        .expect("the settings are valid");

    assert_eq!(roster.logo_codes.len(), 52);
}

#[test]
fn test_roster_new_given_no_seed_builds_an_empty_logo_deck() {
    assert_eq!(roster().logo_codes, Vec::<String>::new());
}

// -------------------------------------------------------------- accessors

#[test]
fn test_roster_entries_returns_the_entries_in_creation_order() {
    let mut roster = roster();
    roster.entries.push(entry("k1", "77A"));
    roster.entries.push(entry("k2", "78B"));

    let keys: Vec<&str> = roster
        .entries()
        .iter()
        .map(|item| item.key.as_str())
        .collect();
    assert_eq!(keys, ["k1", "k2"]);
}

#[test]
fn test_roster_retired_entries_returns_the_retired_entries() {
    let mut roster = roster();
    roster.retired.push(entry("k3", "79C"));

    let keys: Vec<&str> = roster
        .retired_entries()
        .iter()
        .map(|item| item.key.as_str())
        .collect();
    assert_eq!(keys, ["k3"]);
}

#[test]
fn test_roster_audit_log_returns_the_events_oldest_first() {
    let mut roster = roster();
    roster.audit.push(event("first", 1));
    roster.audit.push(event("second", 2));
    roster.audit.push(event("third", 3));

    assert_eq!(
        roster.audit_log(),
        [event("first", 1), event("second", 2), event("third", 3)]
    );
}

#[test]
fn test_roster_audit_log_leaves_the_events_in_place() {
    let mut roster = roster();
    roster.audit.push(event("create_solo_entry", 1));

    let first_read = roster.audit_log().to_vec();
    let second_read = roster.audit_log().to_vec();

    assert_eq!(first_read, second_read);
}

#[test]
fn test_roster_audit_log_given_an_event_keeps_it_available_to_the_next_reader() {
    let mut roster = roster();
    roster.audit.push(event("create_solo_entry", 1));

    let read = roster.audit_log().to_vec();

    assert_eq!(read, [event("create_solo_entry", 1)]);
}

// ---------------------------------------------------------- take_audit_log

#[test]
fn test_roster_take_audit_log_returns_the_events_in_order() {
    let mut roster = roster();
    roster.audit.push(event("first", 1));
    roster.audit.push(event("second", 2));

    let taken = roster.take_audit_log();

    assert_eq!(taken, [event("first", 1), event("second", 2)]);
}

#[test]
fn test_roster_take_audit_log_clears_the_log() {
    let mut roster = roster();
    roster.audit.push(event("first", 1));
    assert_eq!(roster.take_audit_log().len(), 1);

    assert_eq!(roster.audit_log(), []);
}

#[test]
fn test_roster_take_audit_log_given_an_empty_log_returns_no_events() {
    assert_eq!(roster().take_audit_log(), []);
}

// =====================================================================
// Phase 2: creation, editing, the plate namespace and the status setter.
// =====================================================================

use super::{EntryEdit, PlateChange, TeamFields};
use crate::types::{EntryId, Rider};

fn mixed() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, None).expect("the settings are valid")
}

fn relay() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::TeamRelay, None).expect("the settings are valid")
}

fn seeded() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(8843))
        .expect("the settings are valid")
}

fn rider(plate: Option<&str>) -> Rider {
    Rider {
        first_name: "Sam".to_owned(),
        plate: plate.map(str::to_owned),
        ..Rider::default()
    }
}

fn named_rider(first_name: &str, plate: Option<&str>) -> Rider {
    Rider {
        first_name: first_name.to_owned(),
        plate: plate.map(str::to_owned),
        ..Rider::default()
    }
}

fn riders(plates: &[&str]) -> Vec<Rider> {
    plates.iter().map(|plate| rider(Some(plate))).collect()
}

fn team_fields(display_name: &str, team: Vec<Rider>) -> TeamFields<'_> {
    TeamFields {
        display_name,
        riders: team,
        ..TeamFields::default()
    }
}

fn with_plate<'a>(display_name: &'a str, team: Vec<Rider>, plate: &'a str) -> TeamFields<'a> {
    TeamFields {
        display_name,
        riders: team,
        plate: Some(plate),
        ..TeamFields::default()
    }
}

fn created(roster: &Roster, id: &EntryId) -> Entry {
    roster
        .entry_by_key(&id.0)
        .expect("the create path stores its entry")
        .clone()
}

fn log(action: &str, payload: serde_json::Value) -> AuditEvent {
    AuditEvent {
        action: action.to_owned(),
        payload,
    }
}

// ----------------------------------------------------------- create_solo_entry

#[test]
fn test_create_solo_entry_pooled_puts_the_plate_on_entry_and_rider() {
    let mut roster = Roster::new(EntryMode::Solo, 4, PlateModel::RiderPooled, None)
        .expect("the settings are valid");

    let id = roster
        .create_solo_entry(rider(None), "12")
        .expect("a fresh plate is accepted");
    let entry = created(&roster, &id);

    assert_eq!(
        (entry.plate.as_str(), entry.riders[0].plate.as_deref()),
        ("12", Some("12"))
    );
}

#[test]
fn test_create_solo_entry_relay_puts_the_plate_on_the_entry_only() {
    let mut roster = Roster::new(EntryMode::Solo, 4, PlateModel::TeamRelay, None)
        .expect("the settings are valid");

    let id = roster
        .create_solo_entry(rider(None), "12")
        .expect("a fresh plate is accepted");
    let entry = created(&roster, &id);

    assert_eq!(
        (entry.plate.as_str(), entry.riders[0].plate.as_deref()),
        ("12", None)
    );
}

#[test]
fn test_create_solo_entry_sets_display_name_kind_and_defaults() {
    let mut roster = mixed();

    let id = roster
        .create_solo_entry(named_rider("Alex", None), "12")
        .expect("a fresh plate is accepted");
    let entry = created(&roster, &id);

    assert_eq!(
        (
            entry.display_name.as_str(),
            entry.kind,
            entry.status,
            entry.notes.as_str(),
            entry.team_size(),
        ),
        ("Alex", EntryType::Solo, EntryStatus::Active, "", 1)
    );
}

#[test]
fn test_create_solo_entry_given_a_last_name_joins_the_display_name() {
    let mut roster = mixed();
    let alex = Rider {
        first_name: "Alex".to_owned(),
        last_name: "Roy".to_owned(),
        ..Rider::default()
    };

    let id = roster
        .create_solo_entry(alex, "12")
        .expect("a fresh plate is accepted");

    assert_eq!(created(&roster, &id).display_name, "Alex Roy");
}

#[test]
fn test_create_solo_entry_keeps_the_rider_sex() {
    let mut roster = mixed();
    let alex = Rider {
        first_name: "Alex".to_owned(),
        sex: Some("F".to_owned()),
        ..Rider::default()
    };

    let id = roster
        .create_solo_entry(alex, "12")
        .expect("a fresh plate is accepted");

    assert_eq!(created(&roster, &id).riders[0].sex.as_deref(), Some("F"));
}

#[test]
fn test_create_solo_entry_appends_one_audit_event() {
    let mut roster = mixed();

    let id = roster
        .create_solo_entry(named_rider("Alex", None), "12")
        .expect("a fresh plate is accepted");

    assert_eq!(
        roster.audit_log(),
        [log(
            "create_solo_entry",
            serde_json::json!({"plate": "12", "name": "Alex"})
        )]
    );
    assert_eq!(created(&roster, &id).plate, "12");
}

#[test]
fn test_create_solo_entry_given_a_duplicate_plate_refuses() {
    let mut roster = mixed();
    roster
        .create_solo_entry(rider(None), "12")
        .expect("a fresh plate is accepted");

    let error = roster
        .create_solo_entry(rider(None), "12")
        .expect_err("the second claim collides");

    assert_eq!(
        error,
        RosterError::DuplicatePlate {
            plate: "12".to_owned(),
        }
    );
    assert_eq!(roster.entries().len(), 1);
}

#[test]
fn test_create_solo_entry_given_a_letter_plate_refuses() {
    let mut roster = mixed();

    let error = roster
        .create_solo_entry(rider(None), "77A")
        .expect_err("a letter plate is refused");

    assert_eq!(
        error,
        RosterError::PlateShape {
            plate: "77A".to_owned(),
            reason: "must be a whole number".to_owned(),
        }
    );
    assert_eq!(roster.entries(), []);
}

#[test]
fn test_create_solo_entry_relay_given_a_blank_plate_refuses() {
    let mut roster = Roster::new(EntryMode::Solo, 4, PlateModel::TeamRelay, None)
        .expect("the settings are valid");

    let error = roster
        .create_solo_entry(rider(None), "   ")
        .expect_err("a blank plate is refused");

    assert_eq!(
        error,
        RosterError::PlateShape {
            plate: "   ".to_owned(),
            reason: "must not be empty".to_owned(),
        }
    );
    assert_eq!(roster.entries(), []);
}

#[test]
fn test_create_solo_entry_given_the_ride_running_still_creates() {
    let mut roster = mixed();
    roster.set_status(RideStatus::Running);

    let id = roster
        .create_solo_entry(rider(None), "12")
        .expect("a new plate is admitted in any state");

    assert_eq!(created(&roster, &id).plate, "12");
}

// ----------------------------------------------------------- create_team_entry

#[test]
fn test_create_team_entry_pooled_adopts_the_lowest_rider_plate() {
    let mut roster = mixed();

    let id = roster
        .create_team_entry(team_fields("Team A", riders(&["45", "9"])))
        .expect("a valid team is accepted");

    assert_eq!(created(&roster, &id).plate, "9");
}

#[test]
fn test_create_team_entry_relay_clears_every_rider_plate() {
    let mut roster = relay();

    let id = roster
        .create_team_entry(with_plate("Team A", riders(&["5", "6"]), "7"))
        .expect("a valid team is accepted");

    assert_eq!(
        created(&roster, &id)
            .riders
            .iter()
            .map(|rider| rider.plate.clone())
            .collect::<Vec<_>>(),
        [None, None]
    );
}

#[test]
fn test_create_team_entry_sets_display_name_kind_and_defaults() {
    let mut roster = mixed();

    let id = roster
        .create_team_entry(team_fields("Team A", riders(&["1", "2"])))
        .expect("a valid team is accepted");
    let entry = created(&roster, &id);

    assert_eq!(
        (
            entry.display_name.as_str(),
            entry.kind,
            entry.status,
            entry.notes.as_str(),
        ),
        ("Team A", EntryType::Team, EntryStatus::Active, "")
    );
}

#[test]
fn test_create_team_entry_appends_one_audit_event_with_the_team_size() {
    let mut roster = mixed();

    let id = roster
        .create_team_entry(team_fields("Team A", riders(&["1", "2"])))
        .expect("a valid team is accepted");

    assert_eq!(
        roster.audit_log(),
        [log(
            "create_team_entry",
            serde_json::json!({"plate": "1", "display_name": "Team A", "team_size": 2})
        )]
    );
    assert_eq!(created(&roster, &id).team_size(), 2);
}

#[test]
fn test_create_team_entry_given_one_rider_refuses() {
    let mut roster = mixed();

    let error = roster
        .create_team_entry(team_fields("Team A", riders(&["1"])))
        .expect_err("a team below the floor is refused");

    assert_eq!(
        error,
        RosterError::TeamSize {
            message: "team size must be between 2 and 4, got 1".to_owned(),
        }
    );
}

#[test]
fn test_create_team_entry_given_five_riders_refuses() {
    let mut roster = mixed();

    let error = roster
        .create_team_entry(team_fields("Team A", riders(&["1", "2", "3", "4", "5"])))
        .expect_err("a team above the ceiling is refused");

    assert_eq!(
        error,
        RosterError::TeamSize {
            message: "team size must be between 2 and 4, got 5".to_owned(),
        }
    );
}

#[rstest]
#[case::the_minimum(2)]
#[case::the_maximum(4)]
fn test_create_team_entry_given_a_boundary_size_accepts(#[case] size: usize) {
    let plates = ["1", "2", "3", "4"];
    let mut roster = mixed();

    let id = roster
        .create_team_entry(team_fields("Team A", riders(&plates[..size])))
        .expect("a boundary size is accepted");

    assert_eq!(created(&roster, &id).team_size(), size);
}

#[test]
fn test_create_team_entry_on_a_solo_only_ride_refuses() {
    let mut roster = Roster::new(EntryMode::Solo, 4, PlateModel::RiderPooled, None)
        .expect("the settings are valid");

    let error = roster
        .create_team_entry(team_fields("Team A", riders(&["1", "2"])))
        .expect_err("a solo-only ride refuses a team");

    assert_eq!(error, RosterError::SoloOnlyRide);
}

#[test]
fn test_create_team_entry_given_a_duplicate_rider_plate_refuses() {
    let mut roster = mixed();
    roster
        .create_solo_entry(rider(None), "12")
        .expect("a fresh plate is accepted");

    let error = roster
        .create_team_entry(team_fields("Team A", riders(&["12", "13"])))
        .expect_err("the rider plate collides");

    assert_eq!(
        error,
        RosterError::DuplicatePlate {
            plate: "12".to_owned(),
        }
    );
}

#[test]
fn test_create_team_entry_given_two_equal_rider_plates_refuses() {
    let mut roster = mixed();

    let error = roster
        .create_team_entry(team_fields("Team A", riders(&["5", "5"])))
        .expect_err("the team repeats a plate");

    assert_eq!(
        error,
        RosterError::DuplicatePlate {
            plate: "5".to_owned(),
        }
    );
}

#[test]
fn test_create_team_entry_relay_given_no_plate_refuses() {
    let mut roster = relay();

    let error = roster
        .create_team_entry(team_fields("Team A", riders(&["5", "6"])))
        .expect_err("a relay team needs a plate");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "team_relay entries require an explicit plate".to_owned(),
        }
    );
}

#[test]
fn test_create_team_entry_given_the_ride_running_still_creates() {
    let mut roster = mixed();
    roster.set_status(RideStatus::Running);

    let id = roster
        .create_team_entry(team_fields("Team A", riders(&["1", "2"])))
        .expect("the bulk path is not lock-gated");

    assert_eq!(created(&roster, &id).team_size(), 2);
}

#[test]
fn test_create_team_entry_given_a_seed_auto_assigns_the_first_card() {
    let mut roster = seeded();
    let expected = rivercrossing_cards::seeded_card_codes(8843, 1)[0].code();

    let id = roster
        .create_team_entry(team_fields("Team A", riders(&["1", "2"])))
        .expect("a valid team is accepted");

    assert_eq!(created(&roster, &id).logo_card, Some(expected));
}

#[test]
fn test_create_team_entry_given_a_supplied_card_keeps_it() {
    let mut roster = seeded();
    let fields = TeamFields {
        logo_card: Some("AS"),
        ..team_fields("Team A", riders(&["1", "2"]))
    };

    let id = roster
        .create_team_entry(fields)
        .expect("a valid team is accepted");

    assert_eq!(created(&roster, &id).logo_card.as_deref(), Some("AS"));
}

#[test]
fn test_create_team_entry_given_no_seed_assigns_no_card() {
    let mut roster = mixed();

    let id = roster
        .create_team_entry(team_fields("Team A", riders(&["1", "2"])))
        .expect("a valid team is accepted");

    assert_eq!(created(&roster, &id).logo_card, None);
}

#[test]
fn test_create_team_entry_given_two_teams_auto_assigns_distinct_cards() {
    let mut roster = seeded();

    let first = roster
        .create_team_entry(team_fields("Team A", riders(&["1", "2"])))
        .expect("a valid team is accepted");
    let second = roster
        .create_team_entry(team_fields("Team B", riders(&["3", "4"])))
        .expect("a valid team is accepted");

    assert_ne!(
        created(&roster, &first).logo_card,
        created(&roster, &second).logo_card
    );
}

// ---------------------------------------------------- create_team_entry_of_one

#[test]
fn test_create_team_entry_of_one_pooled_adopts_the_riders_plate() {
    let mut roster = mixed();

    let id = roster
        .create_team_entry_of_one(team_fields("Team A", riders(&["7"])))
        .expect("a transient size-1 team is admitted");

    assert_eq!(created(&roster, &id).plate, "7");
}

#[test]
fn test_create_team_entry_of_one_relay_uses_the_given_plate() {
    let mut roster = relay();

    let id = roster
        .create_team_entry_of_one(with_plate("Team A", vec![rider(None)], "7"))
        .expect("a transient size-1 team is admitted");
    let entry = created(&roster, &id);

    assert_eq!(
        (entry.plate.as_str(), entry.riders[0].plate.as_deref()),
        ("7", None)
    );
}

#[test]
fn test_create_team_entry_of_one_appends_one_audit_event() {
    let mut roster = mixed();

    let id = roster
        .create_team_entry_of_one(team_fields("Team A", riders(&["1"])))
        .expect("a transient size-1 team is admitted");

    assert_eq!(
        roster.audit_log(),
        [log(
            "create_team_entry_of_one",
            serde_json::json!({"plate": "1", "display_name": "Team A", "team_size": 1})
        )]
    );
    assert_eq!(created(&roster, &id).team_size(), 1);
}

#[test]
fn test_create_team_entry_of_one_given_two_riders_refuses() {
    let mut roster = mixed();

    let error = roster
        .create_team_entry_of_one(team_fields("Team A", riders(&["1", "2"])))
        .expect_err("the transient path takes exactly one rider");

    assert_eq!(
        error,
        RosterError::TeamSize {
            message: "create_team_entry_of_one takes exactly one rider, got 2".to_owned(),
        }
    );
}

#[test]
fn test_create_team_entry_of_one_given_the_ride_running_refuses() {
    let mut roster = mixed();
    roster.set_status(RideStatus::Running);

    let error = roster
        .create_team_entry_of_one(team_fields("Team A", riders(&["1"])))
        .expect_err("a started ride locks the create");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "a new team cannot be started once the ride is running".to_owned(),
        }
    );
}

#[test]
fn test_create_team_entry_of_one_given_the_ride_finished_refuses_verbatim() {
    let mut roster = mixed();
    roster.set_status(RideStatus::Finished);

    let error = roster
        .create_team_entry_of_one(team_fields("Team A", riders(&["1"])))
        .expect_err("a finished ride locks the create");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "a new team cannot be started once the ride is finished".to_owned(),
        }
    );
}

#[test]
fn test_create_team_entry_of_one_on_a_solo_only_ride_refuses() {
    let mut roster = Roster::new(EntryMode::Solo, 4, PlateModel::RiderPooled, None)
        .expect("the settings are valid");

    let error = roster
        .create_team_entry_of_one(team_fields("Team A", riders(&["1"])))
        .expect_err("a solo-only ride refuses a team");

    assert_eq!(error, RosterError::SoloOnlyRide);
}

#[test]
fn test_create_team_entry_of_one_given_a_seed_auto_assigns_a_card() {
    let mut roster = seeded();
    let expected = rivercrossing_cards::seeded_card_codes(8843, 1)[0].code();

    let id = roster
        .create_team_entry_of_one(team_fields("Team A", riders(&["1"])))
        .expect("a transient size-1 team is admitted");

    assert_eq!(created(&roster, &id).logo_card, Some(expected));
}

// --------------------------------------------------------- create_empty_team

#[test]
fn test_create_empty_team_pooled_claims_the_next_free_plate() {
    let mut roster = mixed();
    roster
        .create_solo_entry(rider(None), "3")
        .expect("a fresh plate is accepted");

    let id = roster
        .create_empty_team(team_fields("Trail Blazers", Vec::new()))
        .expect("an empty pooled team is admitted");
    let entry = created(&roster, &id);

    assert_eq!(
        (entry.display_name.as_str(), entry.kind, entry.team_size()),
        ("Trail Blazers", EntryType::Team, 0)
    );
    assert_eq!(entry.plate, "4");
}

#[test]
fn test_create_empty_team_pooled_given_two_teams_claims_each_next_free() {
    let mut roster = mixed();
    roster
        .create_empty_team(team_fields("Team A", Vec::new()))
        .expect("an empty pooled team is admitted");
    roster
        .create_empty_team(team_fields("Team B", Vec::new()))
        .expect("an empty pooled team is admitted");

    assert_eq!(
        roster
            .entries()
            .iter()
            .map(|entry| entry.plate.clone())
            .collect::<Vec<_>>(),
        ["1", "2"]
    );
}

#[test]
fn test_create_empty_team_relay_uses_the_given_plate() {
    let mut roster = relay();

    let id = roster
        .create_empty_team(with_plate("Trail Blazers", Vec::new(), "120"))
        .expect("an empty relay team is admitted");

    assert_eq!(created(&roster, &id).plate, "120");
}

#[test]
fn test_create_empty_team_relay_given_no_plate_takes_the_next_free() {
    let mut roster = relay();
    roster
        .create_team_entry(with_plate("Moss Ridge", riders(&["1", "2"]), "88"))
        .expect("a valid team is accepted");

    let id = roster
        .create_empty_team(team_fields("Trail Blazers", Vec::new()))
        .expect("an empty relay team is admitted");

    assert_eq!(created(&roster, &id).plate, "89");
}

#[test]
fn test_create_empty_team_pooled_given_an_explicit_plate_refuses() {
    let mut roster = mixed();

    let error = roster
        .create_empty_team(with_plate("Trail Blazers", Vec::new(), "99"))
        .expect_err("a pooled empty team derives no plate from a caller");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "a rider_pooled team's plate is derived from its riders; an empty team claims the next free plate instead"
                .to_owned(),
        }
    );
}

#[test]
fn test_create_empty_team_relay_given_a_duplicate_plate_refuses() {
    let mut roster = relay();
    roster
        .create_team_entry(with_plate("Moss Ridge", riders(&["1", "2"]), "88"))
        .expect("a valid team is accepted");

    let error = roster
        .create_empty_team(with_plate("Trail Blazers", Vec::new(), "88"))
        .expect_err("the plate is already claimed");

    assert_eq!(
        error,
        RosterError::DuplicatePlate {
            plate: "88".to_owned(),
        }
    );
}

#[test]
fn test_create_empty_team_appends_one_audit_event() {
    let mut roster = mixed();

    let id = roster
        .create_empty_team(team_fields("Trail Blazers", Vec::new()))
        .expect("an empty pooled team is admitted");

    assert_eq!(
        roster.audit_log(),
        [log(
            "create_empty_team",
            serde_json::json!({"plate": "1", "display_name": "Trail Blazers", "team_size": 0})
        )]
    );
    assert_eq!(created(&roster, &id).plate, "1");
}

#[test]
fn test_create_empty_team_given_a_rider_refuses() {
    let mut roster = mixed();

    let error = roster
        .create_empty_team(team_fields("Trail Blazers", riders(&["1"])))
        .expect_err("the empty path takes no riders");

    assert_eq!(
        error,
        RosterError::TeamSize {
            message: "create_empty_team takes no riders, got 1".to_owned(),
        }
    );
}

#[test]
fn test_create_empty_team_given_the_ride_running_refuses() {
    let mut roster = mixed();
    roster.set_status(RideStatus::Running);

    let error = roster
        .create_empty_team(team_fields("Trail Blazers", Vec::new()))
        .expect_err("a started ride locks the create");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "a new team cannot be started once the ride is running".to_owned(),
        }
    );
}

#[test]
fn test_create_empty_team_given_the_ride_reopened_refuses_verbatim() {
    let mut roster = mixed();
    roster.set_status(RideStatus::Reopened);

    let error = roster
        .create_empty_team(team_fields("Trail Blazers", Vec::new()))
        .expect_err("a reopened ride locks the create");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "a new team cannot be started once the ride is reopened".to_owned(),
        }
    );
}

#[test]
fn test_create_empty_team_on_a_solo_only_ride_refuses() {
    let mut roster = Roster::new(EntryMode::Solo, 4, PlateModel::RiderPooled, None)
        .expect("the settings are valid");

    let error = roster
        .create_empty_team(team_fields("Trail Blazers", Vec::new()))
        .expect_err("a solo-only ride refuses a team");

    assert_eq!(error, RosterError::SoloOnlyRide);
}

#[test]
fn test_create_empty_team_given_a_seed_auto_assigns_a_card() {
    let mut roster = seeded();
    let expected = rivercrossing_cards::seeded_card_codes(8843, 1)[0].code();

    let id = roster
        .create_empty_team(team_fields("Dirt Dynamos", Vec::new()))
        .expect("an empty pooled team is admitted");

    assert_eq!(created(&roster, &id).logo_card, Some(expected));
}

#[test]
fn test_create_empty_team_given_a_supplied_card_keeps_it() {
    let mut roster = seeded();
    let fields = TeamFields {
        logo_card: Some("AS"),
        ..team_fields("Dirt Dynamos", Vec::new())
    };

    let id = roster
        .create_empty_team(fields)
        .expect("an empty pooled team is admitted");

    assert_eq!(created(&roster, &id).logo_card.as_deref(), Some("AS"));
}

#[test]
fn test_create_empty_team_given_no_seed_assigns_no_card() {
    let mut roster = mixed();

    let id = roster
        .create_empty_team(team_fields("Dirt Dynamos", Vec::new()))
        .expect("an empty pooled team is admitted");

    assert_eq!(created(&roster, &id).logo_card, None);
}

// ------------------------------------------------------------- update_shape

#[test]
fn test_update_shape_replaces_the_three_settings() {
    let mut roster = roster();

    roster
        .update_shape(EntryMode::Mixed, 6, PlateModel::TeamRelay)
        .expect("six is a valid team size");

    assert_eq!(
        (
            roster.entry_mode(),
            roster.max_team_size(),
            roster.plate_model()
        ),
        (EntryMode::Mixed, 6, PlateModel::TeamRelay)
    );
}

#[test]
fn test_update_shape_appends_no_audit_event() {
    let mut roster = roster();

    roster
        .update_shape(EntryMode::Mixed, 6, PlateModel::TeamRelay)
        .expect("six is a valid team size");

    assert_eq!(roster.audit_log(), []);
}

#[test]
fn test_update_shape_leaves_existing_entries_untouched() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    roster
        .update_shape(EntryMode::Mixed, 6, PlateModel::TeamRelay)
        .expect("six is a valid team size");

    let entry = created(&roster, &id);
    assert_eq!(
        (entry.plate.as_str(), entry.riders[0].plate.as_deref()),
        ("1", Some("1"))
    );
}

#[rstest]
#[case::below_the_floor(1)]
#[case::above_the_ceiling(11)]
fn test_update_shape_given_an_out_of_range_size_refuses_and_keeps_the_settings(#[case] size: u32) {
    let mut roster = roster();

    let error = roster
        .update_shape(EntryMode::Mixed, size, PlateModel::TeamRelay)
        .expect_err("the size is outside 2..10");

    assert_eq!(
        error,
        RosterError::TeamSize {
            message: format!("max_team_size must be between 2 and 10, got {size}"),
        }
    );
    assert_eq!(
        (
            roster.entry_mode(),
            roster.max_team_size(),
            roster.plate_model()
        ),
        (EntryMode::Solo, 4, PlateModel::RiderPooled)
    );
}

#[rstest]
#[case::the_minimum(2)]
#[case::min_plus_one(3)]
#[case::max_minus_one(9)]
#[case::the_maximum(10)]
fn test_update_shape_given_an_in_range_size_accepts(#[case] size: u32) {
    let mut roster = roster();

    roster
        .update_shape(EntryMode::Mixed, size, PlateModel::TeamRelay)
        .expect("the size is within 2..10");

    assert_eq!(roster.max_team_size(), size);
}

// -------------------------------------------------------------- update_entry

#[test]
fn test_update_entry_given_a_name_renames_and_keeps_notes() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(named_rider("Alex", None), "1")
        .expect("a fresh plate is accepted");

    roster
        .update_entry(
            &id,
            EntryEdit {
                display_name: Some("Alexandra"),
                ..EntryEdit::default()
            },
        )
        .expect("the entry is a member");

    let entry = created(&roster, &id);
    assert_eq!(
        (entry.display_name.as_str(), entry.notes.as_str()),
        ("Alexandra", "")
    );
}

#[test]
fn test_update_entry_given_notes_edits_notes_and_keeps_the_name() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(named_rider("Alex", None), "1")
        .expect("a fresh plate is accepted");

    roster
        .update_entry(
            &id,
            EntryEdit {
                notes: Some("late scratch"),
                ..EntryEdit::default()
            },
        )
        .expect("the entry is a member");

    let entry = created(&roster, &id);
    assert_eq!(
        (entry.display_name.as_str(), entry.notes.as_str()),
        ("Alex", "late scratch")
    );
}

#[test]
fn test_update_entry_appends_one_audit_event_with_the_changed_fields() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(named_rider("Alex", None), "1")
        .expect("a fresh plate is accepted");
    roster.take_audit_log();

    roster
        .update_entry(
            &id,
            EntryEdit {
                display_name: Some("Alexandra"),
                notes: Some("late scratch"),
            },
        )
        .expect("the entry is a member");

    assert_eq!(
        roster.audit_log(),
        [log(
            "update_entry",
            serde_json::json!({"plate": "1", "display_name": "Alexandra", "notes": "late scratch"})
        )]
    );
}

#[test]
fn test_update_entry_given_a_name_only_audits_only_the_name() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(named_rider("Alex", None), "1")
        .expect("a fresh plate is accepted");
    roster.take_audit_log();

    roster
        .update_entry(
            &id,
            EntryEdit {
                display_name: Some("Alexandra"),
                ..EntryEdit::default()
            },
        )
        .expect("the entry is a member");

    assert_eq!(
        roster.audit_log(),
        [log(
            "update_entry",
            serde_json::json!({"plate": "1", "display_name": "Alexandra"})
        )]
    );
}

#[test]
fn test_update_entry_given_a_retired_entry_updates_it() {
    let mut roster = mixed();
    let retired = Entry::restored("retired1", "88", "Old Team", EntryType::Team, Vec::new());
    roster.retired.push(retired.clone());

    roster
        .update_entry(
            &retired.id(),
            EntryEdit {
                display_name: Some("Renamed Team"),
                ..EntryEdit::default()
            },
        )
        .expect("a retired entry is a member");

    assert_eq!(roster.retired_entries()[0].display_name, "Renamed Team");
}

#[test]
fn test_update_entry_given_a_stranger_refuses() {
    let mut roster = mixed();
    let stranger = Entry::restored("ghost", "999", "Ghost", EntryType::Solo, Vec::new());

    let error = roster
        .update_entry(
            &stranger.id(),
            EntryEdit {
                display_name: Some("Nope"),
                ..EntryEdit::default()
            },
        )
        .expect_err("a foreign entry is refused");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
}

// -------------------------------------------------------------- delete_entry

#[test]
fn test_delete_entry_removes_it_from_entries() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    roster
        .delete_entry(&id)
        .expect("a draft entry is deletable");

    assert_eq!(roster.entries(), []);
}

#[test]
fn test_delete_entry_frees_its_plate_for_reuse() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");
    roster
        .delete_entry(&id)
        .expect("a draft entry is deletable");

    let reused = roster
        .create_solo_entry(rider(None), "1")
        .expect("the plate is free again");

    assert_eq!(created(&roster, &reused).plate, "1");
}

#[test]
fn test_delete_entry_appends_one_audit_event() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(named_rider("Alex", None), "1")
        .expect("a fresh plate is accepted");
    roster.take_audit_log();

    roster
        .delete_entry(&id)
        .expect("a draft entry is deletable");

    assert_eq!(
        roster.audit_log(),
        [log(
            "delete_entry",
            serde_json::json!({"plate": "1", "display_name": "Alex"})
        )]
    );
}

#[rstest]
#[case::running(RideStatus::Running)]
#[case::finished(RideStatus::Finished)]
#[case::reopened(RideStatus::Reopened)]
fn test_delete_entry_given_a_started_ride_refuses(#[case] status: RideStatus) {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");
    roster.set_status(status);

    let error = roster
        .delete_entry(&id)
        .expect_err("a started ride locks the delete");

    assert_eq!(
        error,
        RosterError::Locked {
            message: format!(
                "entries can no longer be deleted once the ride is {}",
                status.as_str()
            ),
        }
    );
}

#[rstest]
#[case::draft(RideStatus::Draft)]
#[case::running(RideStatus::Running)]
#[case::finished(RideStatus::Finished)]
#[case::reopened(RideStatus::Reopened)]
fn test_delete_entry_given_recorded_data_refuses_in_every_state(#[case] status: RideStatus) {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");
    roster.mark_has_data(&id).expect("the entry is a member");
    roster.set_status(status);

    let error = roster
        .delete_entry(&id)
        .expect_err("recorded data blocks the delete");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "entry has recorded data; DNF or void it instead of deleting".to_owned(),
        }
    );
}

#[test]
fn test_delete_entry_given_a_stranger_refuses() {
    let mut roster = mixed();
    let stranger = Entry::restored("ghost", "999", "Ghost", EntryType::Solo, Vec::new());

    let error = roster
        .delete_entry(&stranger.id())
        .expect_err("a foreign entry is refused");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
}

#[test]
fn test_delete_entry_given_a_retired_entry_refuses() {
    let mut roster = mixed();
    let retired = Entry::restored("retired1", "88", "Old Team", EntryType::Team, Vec::new());
    roster.retired.push(retired.clone());

    let error = roster
        .delete_entry(&retired.id())
        .expect_err("a retired entry is not deletable");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "retired1".to_owned(),
        }
    );
}

#[test]
fn test_delete_entry_never_retires_the_entry() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    roster
        .delete_entry(&id)
        .expect("a draft entry is deletable");

    assert_eq!(
        (roster.retired_entries().len(), roster.entries().len()),
        (0, 0)
    );
}

// ------------------------------------------------------------- mark_has_data

#[test]
fn test_mark_has_data_sets_the_flag_and_appends_one_event() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");
    roster.take_audit_log();

    roster.mark_has_data(&id).expect("the entry is a member");

    assert_eq!(
        (created(&roster, &id).has_data, roster.audit_log().to_vec()),
        (
            true,
            vec![log("mark_has_data", serde_json::json!({"plate": "1"}))]
        )
    );
}

#[test]
fn test_mark_has_data_given_a_retired_entry_accepts() {
    let mut roster = mixed();
    let retired = Entry::restored("retired1", "1", "Alex", EntryType::Solo, Vec::new());
    roster.retired.push(retired.clone());

    roster
        .mark_has_data(&retired.id())
        .expect("a retired entry is a member");

    assert_eq!(
        roster.audit_log(),
        [log("mark_has_data", serde_json::json!({"plate": "1"}))]
    );
}

#[test]
fn test_mark_has_data_given_a_stranger_refuses() {
    let mut roster = mixed();
    let stranger = Entry::restored("ghost", "999", "Ghost", EntryType::Solo, Vec::new());

    let error = roster
        .mark_has_data(&stranger.id())
        .expect_err("a foreign entry is refused");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
}

// ------------------------------------------------------------- set_status

#[test]
fn test_set_status_stores_the_new_state() {
    let mut roster = mixed();

    roster.set_status(RideStatus::Reopened);

    assert_eq!(roster.status(), RideStatus::Reopened);
}

// --------------------------------------------------------- change_solo_plate

#[test]
fn test_change_solo_plate_pooled_updates_entry_and_rider() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    roster
        .change_solo_plate(&id, "9")
        .expect("the plate is free");

    let entry = created(&roster, &id);
    assert_eq!(
        (entry.plate.as_str(), entry.riders[0].plate.as_deref()),
        ("9", Some("9"))
    );
}

#[test]
fn test_change_solo_plate_relay_updates_the_entry_only() {
    let mut roster = Roster::new(EntryMode::Solo, 4, PlateModel::TeamRelay, None)
        .expect("the settings are valid");
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    roster
        .change_solo_plate(&id, "9")
        .expect("the plate is free");

    let entry = created(&roster, &id);
    assert_eq!(
        (entry.plate.as_str(), entry.riders[0].plate.as_deref()),
        ("9", None)
    );
}

#[test]
fn test_change_solo_plate_appends_one_audit_event() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(named_rider("Alex", None), "1")
        .expect("a fresh plate is accepted");
    roster.take_audit_log();

    roster
        .change_solo_plate(&id, "9")
        .expect("the plate is free");

    assert_eq!(
        roster.audit_log(),
        [log(
            "change_solo_plate",
            serde_json::json!({"display_name": "Alex", "old_plate": "1", "new_plate": "9", "reason": "1 → 9"})
        )]
    );
}

#[test]
fn test_change_solo_plate_given_its_own_value_assigns_and_audits() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");
    roster.take_audit_log();

    roster
        .change_solo_plate(&id, "1")
        .expect("its own plate is not a collision");

    assert_eq!(
        (created(&roster, &id).plate, roster.audit_log().len()),
        ("1".to_owned(), 1)
    );
}

#[test]
fn test_change_solo_plate_given_the_ride_running_refuses() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");
    roster.set_status(RideStatus::Running);

    let error = roster
        .change_solo_plate(&id, "9")
        .expect_err("plates lock at start");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "plates cannot be changed once the ride is running".to_owned(),
        }
    );
}

#[test]
fn test_change_solo_plate_given_a_duplicate_refuses() {
    let mut roster = mixed();
    roster
        .create_solo_entry(rider(None), "2")
        .expect("a fresh plate is accepted");
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    let error = roster
        .change_solo_plate(&id, "2")
        .expect_err("the plate is claimed");

    assert_eq!(
        error,
        RosterError::DuplicatePlate {
            plate: "2".to_owned(),
        }
    );
}

#[test]
fn test_change_solo_plate_given_a_team_refuses() {
    let mut roster = mixed();
    let id = roster
        .create_team_entry(team_fields("Team A", riders(&["1", "2"])))
        .expect("a valid team is accepted");

    let error = roster
        .change_solo_plate(&id, "9")
        .expect_err("a team is not a solo entry");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "change_solo_plate requires a solo entry".to_owned(),
        }
    );
}

#[test]
fn test_change_solo_plate_given_a_stranger_refuses() {
    let mut roster = mixed();
    let stranger = Entry::restored("ghost", "999", "Ghost", EntryType::Solo, Vec::new());

    let error = roster
        .change_solo_plate(&stranger.id(), "9")
        .expect_err("a foreign entry is refused");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
}

#[test]
fn test_change_solo_plate_given_a_blank_refuses_and_changes_nothing() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    let error = roster
        .change_solo_plate(&id, "")
        .expect_err("a blank plate is refused");

    assert_eq!(
        error,
        RosterError::PlateShape {
            plate: String::new(),
            reason: "must not be empty".to_owned(),
        }
    );
    assert_eq!(created(&roster, &id).plate, "1");
}

#[test]
fn test_change_solo_plate_given_a_letter_refuses_and_changes_nothing() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    let error = roster
        .change_solo_plate(&id, "77A")
        .expect_err("a letter plate is refused");

    assert_eq!(
        error,
        RosterError::PlateShape {
            plate: "77A".to_owned(),
            reason: "must be a whole number".to_owned(),
        }
    );
    assert_eq!(created(&roster, &id).plate, "1");
}

// ------------------------------------------------ change_pooled_rider_plate

#[test]
fn test_change_pooled_rider_plate_updates_the_rider_and_recomputes_the_plate() {
    let mut roster = mixed();
    let id = roster
        .create_team_entry(team_fields("Team A", riders(&["5", "9"])))
        .expect("a valid team is accepted");

    roster
        .change_pooled_rider_plate("5", "1")
        .expect("the plate is free");

    let entry = created(&roster, &id);
    assert_eq!(
        (entry.riders[0].plate.as_deref(), entry.plate.as_str()),
        (Some("1"), "1")
    );
}

#[test]
fn test_change_pooled_rider_plate_appends_one_audit_event() {
    let mut roster = mixed();
    roster
        .create_team_entry(team_fields(
            "Team A",
            vec![named_rider("Alex", Some("5")), rider(Some("9"))],
        ))
        .expect("a valid team is accepted");
    roster.take_audit_log();

    roster
        .change_pooled_rider_plate("5", "1")
        .expect("the plate is free");

    assert_eq!(
        roster.audit_log(),
        [log(
            "change_pooled_rider_plate",
            serde_json::json!({"rider_name": "Alex", "old_plate": "5", "new_plate": "1", "reason": "5 → 1"})
        )]
    );
}

#[test]
fn test_change_pooled_rider_plate_given_its_own_value_assigns_and_audits() {
    let mut roster = mixed();
    roster
        .create_team_entry(team_fields("Team A", riders(&["5", "9"])))
        .expect("a valid team is accepted");
    roster.take_audit_log();

    roster
        .change_pooled_rider_plate("5", "5")
        .expect("its own plate is not a collision");

    assert_eq!(roster.audit_log().len(), 1);
}

#[test]
fn test_change_pooled_rider_plate_given_the_ride_running_refuses() {
    let mut roster = mixed();
    roster
        .create_team_entry(team_fields("Team A", riders(&["5", "9"])))
        .expect("a valid team is accepted");
    roster.set_status(RideStatus::Running);

    let error = roster
        .change_pooled_rider_plate("5", "1")
        .expect_err("plates lock at start");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "plates cannot be changed once the ride is running".to_owned(),
        }
    );
}

#[test]
fn test_change_pooled_rider_plate_given_a_duplicate_refuses() {
    let mut roster = mixed();
    roster
        .create_team_entry(team_fields("Team A", riders(&["5", "9"])))
        .expect("a valid team is accepted");
    roster
        .create_solo_entry(rider(None), "3")
        .expect("a fresh plate is accepted");

    let error = roster
        .change_pooled_rider_plate("5", "3")
        .expect_err("the plate is claimed");

    assert_eq!(
        error,
        RosterError::DuplicatePlate {
            plate: "3".to_owned(),
        }
    );
}

#[test]
fn test_change_pooled_rider_plate_on_a_relay_ride_refuses() {
    let mut roster = relay();
    roster.entries.push(Entry::restored(
        "relay1",
        "7",
        "Team A",
        EntryType::Team,
        vec![rider(Some("5")), rider(Some("6"))],
    ));

    let error = roster
        .change_pooled_rider_plate("5", "9")
        .expect_err("a relay ride has no pooled member plate");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "change_pooled_rider_plate requires a rider_pooled team member".to_owned(),
        }
    );
}

#[test]
fn test_change_pooled_rider_plate_given_a_solo_rider_refuses() {
    let mut roster = mixed();
    roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    let error = roster
        .change_pooled_rider_plate("1", "9")
        .expect_err("a solo rider is out of scope");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "change_pooled_rider_plate requires a rider_pooled team member".to_owned(),
        }
    );
}

#[test]
fn test_change_pooled_rider_plate_given_a_stranger_refuses() {
    let mut roster = mixed();
    roster
        .create_team_entry(team_fields("Team A", riders(&["1", "2"])))
        .expect("a valid team is accepted");

    let error = roster
        .change_pooled_rider_plate("999", "9")
        .expect_err("the rider is on no entry");

    assert_eq!(
        error,
        RosterError::RiderNotFound {
            description: "rider is not on any entry in this roster".to_owned(),
        }
    );
}

#[test]
fn test_change_pooled_rider_plate_given_a_letter_refuses_and_changes_nothing() {
    let mut roster = mixed();
    let id = roster
        .create_team_entry(team_fields("Team A", riders(&["3", "9"])))
        .expect("a valid team is accepted");

    let error = roster
        .change_pooled_rider_plate("3", "77A")
        .expect_err("a letter plate is refused");

    assert_eq!(
        error,
        RosterError::PlateShape {
            plate: "77A".to_owned(),
            reason: "must be a whole number".to_owned(),
        }
    );
    let entry = created(&roster, &id);
    assert_eq!(
        (entry.riders[0].plate.as_deref(), entry.plate.as_str()),
        (Some("3"), "3")
    );
}

// --------------------------------------------------------- change_team_plate

#[test]
fn test_change_team_plate_updates_the_entry_only() {
    let mut roster = relay();
    let id = roster
        .create_team_entry(with_plate("Team A", riders(&["1", "2"]), "1"))
        .expect("a valid team is accepted");

    roster
        .change_team_plate(&id, "9")
        .expect("the plate is free");

    let entry = created(&roster, &id);
    assert_eq!(
        (entry.plate.as_str(), entry.riders[0].plate.as_deref()),
        ("9", None)
    );
}

#[test]
fn test_change_team_plate_appends_one_audit_event() {
    let mut roster = relay();
    let id = roster
        .create_team_entry(with_plate("Team A", riders(&["1", "2"]), "1"))
        .expect("a valid team is accepted");
    roster.take_audit_log();

    roster
        .change_team_plate(&id, "9")
        .expect("the plate is free");

    assert_eq!(
        roster.audit_log(),
        [log(
            "change_team_plate",
            serde_json::json!({"display_name": "Team A", "old_plate": "1", "new_plate": "9", "reason": "1 → 9"})
        )]
    );
}

#[test]
fn test_change_team_plate_given_its_own_value_assigns_and_audits() {
    let mut roster = relay();
    let id = roster
        .create_team_entry(with_plate("Team A", riders(&["1", "2"]), "1"))
        .expect("a valid team is accepted");
    roster.take_audit_log();

    roster
        .change_team_plate(&id, "1")
        .expect("its own plate is not a collision");

    assert_eq!(roster.audit_log().len(), 1);
}

#[test]
fn test_change_team_plate_given_the_ride_running_refuses() {
    let mut roster = relay();
    let id = roster
        .create_team_entry(with_plate("Team A", riders(&["1", "2"]), "1"))
        .expect("a valid team is accepted");
    roster.set_status(RideStatus::Running);

    let error = roster
        .change_team_plate(&id, "9")
        .expect_err("plates lock at start");

    assert_eq!(
        error,
        RosterError::Locked {
            message: "plates cannot be changed once the ride is running".to_owned(),
        }
    );
}

#[test]
fn test_change_team_plate_given_a_duplicate_refuses() {
    let mut roster = relay();
    roster
        .create_team_entry(with_plate("Team B", riders(&["5", "6"]), "5"))
        .expect("a valid team is accepted");
    let id = roster
        .create_team_entry(with_plate("Team A", riders(&["1", "2"]), "1"))
        .expect("a valid team is accepted");

    let error = roster
        .change_team_plate(&id, "5")
        .expect_err("the plate is claimed");

    assert_eq!(
        error,
        RosterError::DuplicatePlate {
            plate: "5".to_owned(),
        }
    );
}

#[test]
fn test_change_team_plate_on_a_pooled_team_refuses() {
    let mut roster = mixed();
    let id = roster
        .create_team_entry(team_fields("Team A", riders(&["1", "2"])))
        .expect("a valid team is accepted");

    let error = roster
        .change_team_plate(&id, "9")
        .expect_err("a pooled team's plate is derived");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "a rider_pooled team's plate is derived from its riders; use change_pooled_rider_plate instead"
                .to_owned(),
        }
    );
}

#[test]
fn test_change_team_plate_given_a_solo_entry_refuses() {
    let mut roster = relay();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    let error = roster
        .change_team_plate(&id, "9")
        .expect_err("a solo entry is not a team");

    assert_eq!(
        error,
        RosterError::InvalidMove {
            message: "change_team_plate requires a team entry; use change_solo_plate".to_owned(),
        }
    );
}

#[test]
fn test_change_team_plate_given_a_stranger_refuses() {
    let mut roster = relay();
    let stranger = Entry::restored("ghost", "999", "Ghost", EntryType::Team, Vec::new());

    let error = roster
        .change_team_plate(&stranger.id(), "9")
        .expect_err("a foreign entry is refused");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
}

#[test]
fn test_change_team_plate_given_a_blank_refuses_and_changes_nothing() {
    let mut roster = relay();
    let id = roster
        .create_team_entry(with_plate("Team A", riders(&["1", "2"]), "77"))
        .expect("a valid team is accepted");

    let error = roster
        .change_team_plate(&id, "")
        .expect_err("a blank plate is refused");

    assert_eq!(
        error,
        RosterError::PlateShape {
            plate: String::new(),
            reason: "must not be empty".to_owned(),
        }
    );
    assert_eq!(created(&roster, &id).plate, "77");
}

// -------------------------------------------------------------- change_plate

#[test]
fn test_change_plate_solo_dispatches_to_change_solo_plate() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");
    roster.take_audit_log();

    roster
        .change_plate(PlateChange {
            entry: &id,
            rider_plate: Some("1"),
            plate: "9",
        })
        .expect("the plate is free");

    let entry = created(&roster, &id);
    assert_eq!(
        (
            roster.audit_log()[0].action.as_str(),
            entry.plate.as_str(),
            entry.riders[0].plate.as_deref()
        ),
        ("change_solo_plate", "9", Some("9"))
    );
}

#[test]
fn test_change_plate_pooled_member_dispatches_to_change_pooled_rider_plate() {
    let mut roster = mixed();
    let id = roster
        .create_team_entry(team_fields("Trail Blazers", riders(&["1", "2"])))
        .expect("a valid team is accepted");
    roster.take_audit_log();

    roster
        .change_plate(PlateChange {
            entry: &id,
            rider_plate: Some("2"),
            plate: "9",
        })
        .expect("the plate is free");

    let entry = created(&roster, &id);
    assert_eq!(
        (
            roster.audit_log()[0].action.as_str(),
            entry.riders[1].plate.as_deref(),
            entry.plate.as_str()
        ),
        ("change_pooled_rider_plate", Some("9"), "1")
    );
}

#[test]
fn test_change_plate_relay_team_dispatches_to_change_team_plate() {
    let mut roster = relay();
    let id = roster
        .create_team_entry(with_plate("Trail Blazers", riders(&["1", "2"]), "5"))
        .expect("a valid team is accepted");
    roster.take_audit_log();

    roster
        .change_plate(PlateChange {
            entry: &id,
            rider_plate: None,
            plate: "9",
        })
        .expect("the plate is free");

    assert_eq!(
        (
            roster.audit_log()[0].action.as_str(),
            created(&roster, &id).plate.as_str()
        ),
        ("change_team_plate", "9")
    );
}

#[test]
fn test_change_plate_given_its_own_value_is_a_no_op() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");
    roster.take_audit_log();

    roster
        .change_plate(PlateChange {
            entry: &id,
            rider_plate: Some("1"),
            plate: "1",
        })
        .expect("an unchanged plate is nothing to do");

    assert_eq!(roster.audit_log(), []);
}

#[test]
fn test_change_plate_pooled_given_its_own_value_is_a_no_op() {
    let mut roster = mixed();
    let id = roster
        .create_team_entry(team_fields("Trail Blazers", riders(&["1", "2"])))
        .expect("a valid team is accepted");
    roster.take_audit_log();

    roster
        .change_plate(PlateChange {
            entry: &id,
            rider_plate: Some("2"),
            plate: "2",
        })
        .expect("an unchanged plate is nothing to do");

    assert_eq!(roster.audit_log(), []);
}

#[test]
fn test_change_plate_relay_given_its_own_value_is_a_no_op() {
    let mut roster = relay();
    let id = roster
        .create_team_entry(with_plate("Trail Blazers", riders(&["1", "2"]), "5"))
        .expect("a valid team is accepted");
    roster.take_audit_log();

    roster
        .change_plate(PlateChange {
            entry: &id,
            rider_plate: None,
            plate: "5",
        })
        .expect("an unchanged plate is nothing to do");

    assert_eq!(roster.audit_log(), []);
}

#[test]
fn test_change_plate_pooled_given_no_rider_refuses() {
    let mut roster = mixed();
    let id = roster
        .create_team_entry(team_fields("Trail Blazers", riders(&["1", "2"])))
        .expect("a valid team is accepted");

    let error = roster
        .change_plate(PlateChange {
            entry: &id,
            rider_plate: None,
            plate: "9",
        })
        .expect_err("a pooled team needs a named member");

    assert_eq!(
        error,
        RosterError::RiderNotFound {
            description: "a rider_pooled team plate change requires the member rider".to_owned(),
        }
    );
}

#[test]
fn test_change_plate_given_a_stranger_refuses() {
    let mut roster = mixed();
    let stranger = Entry::restored("ghost", "999", "Ghost", EntryType::Solo, Vec::new());

    let error = roster
        .change_plate(PlateChange {
            entry: &stranger.id(),
            rider_plate: Some("999"),
            plate: "9",
        })
        .expect_err("a foreign entry is refused");

    assert_eq!(
        error,
        RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
}

#[test]
fn test_change_plate_pooled_given_an_unknown_rider_plate_refuses() {
    let mut roster = mixed();
    let id = roster
        .create_team_entry(team_fields("Trail Blazers", riders(&["1", "2"])))
        .expect("a valid team is accepted");

    let error = roster
        .change_plate(PlateChange {
            entry: &id,
            rider_plate: Some("999"),
            plate: "9",
        })
        .expect_err("the rider is on no entry");

    assert_eq!(
        error,
        RosterError::RiderNotFound {
            description: "rider is not on any entry in this roster".to_owned(),
        }
    );
}

#[test]
fn test_change_plate_solo_given_a_blank_plate_refuses() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    let error = roster
        .change_plate(PlateChange {
            entry: &id,
            rider_plate: Some("1"),
            plate: "   ",
        })
        .expect_err("a blank plate is refused");

    assert_eq!(
        error,
        RosterError::PlateShape {
            plate: "   ".to_owned(),
            reason: "must not be empty".to_owned(),
        }
    );
}

// ------------------------------------------------- plate and key lookups

#[test]
fn test_next_free_plate_spans_entry_and_rider_plates() {
    let mut roster = mixed();
    roster
        .create_team_entry(team_fields("Team A", riders(&["3", "50"])))
        .expect("a valid team is accepted");

    assert_eq!(roster.next_free_plate(), "51");
}

#[test]
fn test_resolve_plate_given_a_pooled_rider_plate_returns_the_entry() {
    let mut roster = mixed();
    let id = roster
        .create_team_entry(team_fields("Team A", riders(&["45", "9"])))
        .expect("a valid team is accepted");

    assert_eq!(
        roster.resolve_plate("45").map(|entry| entry.key.clone()),
        Some(id.0.clone())
    );
}

#[test]
fn test_entry_by_key_returns_a_live_entry() {
    let mut roster = mixed();
    let id = roster
        .create_solo_entry(rider(None), "12")
        .expect("a fresh plate is accepted");

    assert_eq!(
        roster.entry_by_key(&id.0).map(|entry| entry.plate.clone()),
        Some("12".to_owned())
    );
}

#[test]
fn test_entry_by_key_returns_a_retired_entry() {
    let mut roster = mixed();
    let retired = Entry::restored("retired1", "88", "Old Team", EntryType::Team, Vec::new());
    roster.retired.push(retired.clone());

    assert_eq!(
        roster.entry_by_key("retired1").map(Entry::id),
        Some(retired.id())
    );
}

#[test]
fn test_entry_by_key_given_an_unknown_key_returns_none() {
    assert_eq!(mixed().entry_by_key("missing"), None);
}

#[test]
fn test_next_team_logo_card_given_no_seed_returns_none() {
    assert_eq!(mixed().next_team_logo_card(None), None);
}

#[test]
fn test_next_team_logo_card_given_the_last_card_wraps_to_the_first() {
    let mut roster = seeded();
    let last = rivercrossing_cards::seeded_card_codes(8843, 1)[51].code();
    let first = rivercrossing_cards::seeded_card_codes(8843, 1)[0].code();
    let fields = TeamFields {
        logo_card: Some(last.as_str()),
        ..team_fields("Team A", riders(&["1", "2"]))
    };
    roster
        .create_team_entry(fields)
        .expect("a valid team is accepted");

    assert_eq!(roster.next_team_logo_card(Some(&last)), Some(first));
}

// ------------------------------------------------------------- load seam

/// Build *count* solo entries whose plates count up from zero.
fn numbered_entries(count: usize) -> Vec<Entry> {
    (0..count)
        .map(|number| {
            Entry::restored(
                format!("key-{number}"),
                number.to_string(),
                format!("Rider {number}"),
                EntryType::Solo,
                Vec::new(),
            )
        })
        .collect()
}

#[test]
fn test_load_entries_given_no_entries_leaves_the_list_empty() {
    let mut roster = mixed();

    roster.load_entries(numbered_entries(0));

    assert_eq!(roster.entries(), []);
}

#[test]
fn test_load_entries_given_one_entry_appends_it() {
    let mut roster = mixed();

    roster.load_entries(numbered_entries(1));

    assert_eq!(
        roster
            .entries()
            .iter()
            .map(|entry| entry.plate.as_str())
            .collect::<Vec<_>>(),
        ["0"]
    );
}

#[test]
fn test_load_entries_given_three_entries_appends_them_in_order() {
    let mut roster = mixed();

    roster.load_entries(numbered_entries(3));

    assert_eq!(
        roster
            .entries()
            .iter()
            .map(|entry| entry.plate.as_str())
            .collect::<Vec<_>>(),
        ["0", "1", "2"]
    );
}

#[test]
fn test_load_entries_keeps_the_existing_entries() {
    let mut roster = mixed();
    let live = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    roster.load_entries(numbered_entries(1));

    assert_eq!(
        roster
            .entries()
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<Vec<_>>(),
        [live.0, "key-0".to_owned()]
    );
}

#[test]
fn test_load_entries_does_not_audit() {
    let mut roster = mixed();

    roster.load_entries(numbered_entries(2));

    assert_eq!(roster.audit_log(), []);
}

#[test]
fn test_load_entries_does_not_validate_a_repeated_plate() {
    let mut roster = mixed();
    let mut first = entry("first", "7");
    let second = entry("second", "7");
    first.has_data = true;

    roster.load_entries(vec![first, second]);

    assert_eq!(roster.entries().len(), 2);
}

#[test]
fn test_load_entries_keeps_the_retired_entries_alone() {
    let mut roster = mixed();
    let retired = entry("retired1", "88");
    roster.load_retired_entries(vec![retired]);

    roster.load_entries(numbered_entries(1));

    assert_eq!(
        roster
            .retired_entries()
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>(),
        ["retired1"]
    );
}

#[test]
fn test_load_retired_entries_given_no_entries_leaves_the_list_empty() {
    let mut roster = mixed();

    roster.load_retired_entries(numbered_entries(0));

    assert_eq!(roster.retired_entries(), []);
}

#[test]
fn test_load_retired_entries_given_one_entry_appends_it() {
    let mut roster = mixed();

    roster.load_retired_entries(numbered_entries(1));

    assert_eq!(
        roster
            .retired_entries()
            .iter()
            .map(|entry| entry.plate.as_str())
            .collect::<Vec<_>>(),
        ["0"]
    );
}

#[test]
fn test_load_retired_entries_given_three_entries_appends_them_in_order() {
    let mut roster = mixed();

    roster.load_retired_entries(numbered_entries(3));

    assert_eq!(
        roster
            .retired_entries()
            .iter()
            .map(|entry| entry.plate.as_str())
            .collect::<Vec<_>>(),
        ["0", "1", "2"]
    );
}

#[test]
fn test_load_retired_entries_does_not_audit() {
    let mut roster = mixed();

    roster.load_retired_entries(numbered_entries(2));

    assert_eq!(roster.audit_log(), []);
}

#[test]
fn test_load_retired_entries_keeps_the_live_entries_alone() {
    let mut roster = mixed();
    let live = roster
        .create_solo_entry(rider(None), "1")
        .expect("a fresh plate is accepted");

    roster.load_retired_entries(numbered_entries(1));

    assert_eq!(
        roster
            .entries()
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>(),
        [live.0.as_str()]
    );
}

// ------------------------------------------------------------- dissolve

#[test]
fn test_dissolve_emptied_given_an_unknown_key_leaves_both_lists_alone() {
    let mut roster = mixed();
    roster.load_entries(numbered_entries(1));

    roster.dissolve_emptied(&EntryId("ghost".to_owned()));

    assert_eq!(
        (
            roster.entries().len(),
            roster.retired_entries().len(),
            roster.audit_log().len()
        ),
        (1, 0, 0)
    );
}
