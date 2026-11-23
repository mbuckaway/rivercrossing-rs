// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the `RosterView` implementation that the ride engine reads.

use pretty_assertions::assert_eq;
use rivercrossing_ride::{EntryId, RideError, RiderSnapshot, RosterView, StartViolation};

use crate::enums::{EntryMode, EntryStatus, EntryType, PlateModel, RideStatus};
use crate::roster::{EntryEdit, Roster, TeamFields};
use crate::types::{Entry, Rider};

fn pooled() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, None)
        .expect("the default settings are valid")
}

fn relay() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::TeamRelay, None)
        .expect("the default settings are valid")
}

fn rider(first_name: &str, last_name: &str, plate: Option<&str>) -> Rider {
    Rider {
        first_name: first_name.to_owned(),
        last_name: last_name.to_owned(),
        plate: plate.map(str::to_owned),
        ..Rider::default()
    }
}

fn member(first_name: &str, plate: &str) -> Rider {
    rider(first_name, "", Some(plate))
}

fn team(roster: &mut Roster, display_name: &str, members: &[(&str, &str)]) -> EntryId {
    let riders = members
        .iter()
        .map(|(first_name, plate)| member(first_name, plate))
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
            riders: vec![rider("Alex", "", None), rider("Bo", "", None)],
            plate: Some(plate),
            ..TeamFields::default()
        })
        .expect("a valid relay team is accepted")
}

fn team_of_one(roster: &mut Roster, display_name: &str, plate: &str) -> EntryId {
    roster
        .create_team_entry_of_one(TeamFields {
            display_name,
            riders: vec![member("Alex", plate)],
            ..TeamFields::default()
        })
        .expect("a draft team of one is accepted")
}

#[expect(
    clippy::too_many_arguments,
    reason = "the fixture keeps the rider's two name parts and plate explicit; a struct would bury the one-line call sites"
)]
fn solo(roster: &mut Roster, first_name: &str, last_name: &str, plate: &str) -> EntryId {
    roster
        .create_solo_entry(rider(first_name, last_name, None), plate)
        .expect("a fresh plate is accepted")
}

fn retired(key: &str, plate: &str, display_name: &str) -> Entry {
    Entry::restored(
        key,
        plate,
        display_name,
        EntryType::Team,
        vec![rider("Cy", "", None)],
    )
}

/// Create a solo entry, flag it as carrying data, and move its rider onto a new
/// team. The emptied solo entry retires, so its key stays resolvable.
fn dissolved_solo(roster: &mut Roster) -> EntryId {
    let id = solo(roster, "Ada", "Lovelace", "1");
    roster.mark_has_data(&id).expect("the solo entry is live");
    let destination = team(roster, "Team B", &[("Cy", "10"), ("Do", "11")]);
    roster
        .move_rider("1", &destination, "rider moved out")
        .expect("the move is allowed");
    id
}

// ----------------------------------------------------------------- entries

#[test]
fn test_view_entries_given_an_empty_roster_returns_an_empty_list() {
    let roster = pooled();

    assert_eq!(RosterView::entries(&roster), []);
}

#[test]
fn test_view_entries_returns_live_entries_in_creation_order() {
    let mut roster = pooled();
    let first = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let second = solo(&mut roster, "Ada", "Lovelace", "3");
    let third = team(&mut roster, "Team C", &[("Cy", "10"), ("Do", "11")]);

    let ids: Vec<EntryId> = RosterView::entries(&roster)
        .into_iter()
        .map(|snapshot| snapshot.id)
        .collect();
    assert_eq!(ids, [first, second, third]);
}

#[test]
fn test_view_entries_maps_a_team_to_a_snapshot() {
    let mut roster = pooled();
    let id = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    let snapshots = RosterView::entries(&roster);
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].id, id);
    assert_eq!(snapshots[0].plate, "1");
    assert_eq!(snapshots[0].display_name, "Team A");
    assert_eq!((snapshots[0].is_team, snapshots[0].is_dnf), (true, false));
    assert_eq!(snapshots[0].riders.len(), 2);
}

#[test]
fn test_view_entries_maps_a_solo_to_a_snapshot() {
    let mut roster = pooled();
    let id = solo(&mut roster, "Ada", "Lovelace", "3");

    let snapshots = RosterView::entries(&roster);
    assert_eq!(snapshots[0].id, id);
    assert_eq!(snapshots[0].plate, "3");
    assert_eq!(snapshots[0].display_name, "Ada Lovelace");
    assert_eq!((snapshots[0].is_team, snapshots[0].is_dnf), (false, false));
}

#[test]
fn test_view_entries_uses_the_stored_display_name() {
    let mut roster = pooled();
    let id = solo(&mut roster, "Ada", "Lovelace", "3");
    roster
        .update_entry(
            &id,
            EntryEdit {
                display_name: Some("Ace"),
                ..EntryEdit::default()
            },
        )
        .expect("the entry is known");

    assert_eq!(RosterView::entries(&roster)[0].display_name, "Ace");
}

#[test]
fn test_view_entries_maps_each_rider_to_a_snapshot() {
    let mut roster = pooled();
    let faithful = Rider {
        first_name: "Ada".to_owned(),
        last_name: "Lovelace".to_owned(),
        sex: Some("F".to_owned()),
        ..Rider::default()
    };
    roster
        .create_solo_entry(faithful, "3")
        .expect("a fresh plate is accepted");

    assert_eq!(
        RosterView::entries(&roster)[0].riders,
        [RiderSnapshot {
            plate: Some("3".to_owned()),
            full_name: "Ada Lovelace".to_owned(),
            sex: Some("F".to_owned()),
        }]
    );
}

#[test]
fn test_view_entries_maps_a_relay_rider_without_a_plate() {
    let mut roster = relay();
    relay_team(&mut roster, "Team A", "7");

    let riders = &RosterView::entries(&roster)[0].riders;
    assert_eq!(riders.len(), 2);
    assert_eq!(riders[0].plate, None);
    assert_eq!(riders[1].plate, None);
}

#[test]
fn test_view_entries_reports_a_dnf_entry() {
    let mut roster = pooled();
    let id = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    RosterView::set_entry_dnf(&mut roster, &id, "1", false, "bike broke");

    assert_eq!(RosterView::entries(&roster)[0].is_dnf, true);
}

#[test]
fn test_view_entries_excludes_a_retired_entry() {
    let mut roster = pooled();
    team(&mut roster, "Live", &[("Alex", "1"), ("Bo", "2")]);
    roster.load_retired_entries(vec![retired("r1", "9", "Gone")]);

    let snapshots = RosterView::entries(&roster);
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].display_name, "Live");
}

#[test]
fn test_view_entries_drops_a_dissolved_entry() {
    let mut roster = pooled();
    let dissolved = dissolved_solo(&mut roster);

    let snapshots = RosterView::entries(&roster);
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].display_name, "Team B");
    assert_eq!(
        RosterView::entry(&roster, &dissolved).map(|snap| snap.id),
        Some(dissolved)
    );
}

// ------------------------------------------------------------- resolve_plate

#[test]
fn test_view_resolve_plate_resolves_an_entry_plate() {
    let mut roster = pooled();
    let id = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    assert_eq!(
        RosterView::resolve_plate(&roster, "1").map(|snap| snap.id),
        Some(id)
    );
}

#[test]
fn test_view_resolve_plate_resolves_a_pooled_rider_plate() {
    let mut roster = pooled();
    let id = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    let snapshot = RosterView::resolve_plate(&roster, "2").expect("a pooled member resolves");
    assert_eq!(snapshot.id, id);
    assert_eq!(snapshot.plate, "1");
}

#[test]
fn test_view_resolve_plate_given_a_relay_rider_plate_returns_none() {
    let mut roster = relay();
    roster.load_entries(vec![Entry::restored(
        "k1",
        "7",
        "Relay",
        EntryType::Team,
        vec![rider("Cy", "", Some("8"))],
    )]);

    assert_eq!(RosterView::resolve_plate(&roster, "8"), None);
    assert_eq!(
        RosterView::resolve_plate(&roster, "7").map(|snap| snap.id),
        Some(EntryId("k1".to_owned()))
    );
}

#[test]
fn test_view_resolve_plate_given_an_unknown_plate_returns_none() {
    let roster = pooled();

    assert_eq!(RosterView::resolve_plate(&roster, "99"), None);
}

#[test]
fn test_view_resolve_plate_given_a_retired_entry_plate_returns_none() {
    let mut roster = pooled();
    roster.load_retired_entries(vec![retired("r1", "99", "Gone")]);

    assert_eq!(RosterView::resolve_plate(&roster, "99"), None);
}

// ------------------------------------------------------------------- entry

#[test]
fn test_view_entry_resolves_a_live_key() {
    let mut roster = pooled();
    let id = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    assert_eq!(
        RosterView::entry(&roster, &id).map(|snap| snap.display_name),
        Some("Team A".to_owned())
    );
}

#[test]
fn test_view_entry_resolves_a_retired_key() {
    let mut roster = pooled();
    let id = dissolved_solo(&mut roster);

    assert_eq!(
        RosterView::entry(&roster, &id).map(|snap| snap.id),
        Some(id)
    );
}

#[test]
fn test_view_entry_given_an_unknown_key_returns_none() {
    let roster = pooled();

    assert_eq!(
        RosterView::entry(&roster, &EntryId("missing".to_owned())),
        None
    );
}

// -------------------------------------------------------- start_violations

#[test]
fn test_view_start_violations_equals_validate_for_start() {
    let mut roster = pooled();
    team_of_one(&mut roster, "Team A", "1");

    assert_eq!(
        RosterView::start_violations(&roster),
        roster.validate_for_start()
    );
}

#[test]
fn test_view_start_violations_given_a_clean_roster_is_empty() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    assert_eq!(
        RosterView::start_violations(&roster),
        Vec::<StartViolation>::new()
    );
}

// ---------------------------------------------------------- set_ride_status

#[test]
fn test_view_set_ride_status_records_the_status() {
    let mut roster = pooled();

    RosterView::set_ride_status(&mut roster, RideStatus::Running);

    assert_eq!(roster.status(), RideStatus::Running);
}

// ----------------------------------------------------------- mark_has_data

#[test]
fn test_view_mark_has_data_sets_the_entry_flag() {
    let mut roster = pooled();
    let id = solo(&mut roster, "Ada", "Lovelace", "3");

    RosterView::mark_has_data(&mut roster, &id);

    assert_eq!(
        roster
            .entry_by_key(&id.0)
            .expect("the entry is known")
            .has_data,
        true
    );
}

#[test]
fn test_view_mark_has_data_accepts_a_retired_entry() {
    let mut roster = pooled();
    roster.load_retired_entries(vec![retired("r1", "9", "Gone")]);

    RosterView::mark_has_data(&mut roster, &EntryId("r1".to_owned()));

    assert_eq!(roster.retired_entries()[0].has_data, true);
}

#[test]
fn test_view_mark_has_data_given_an_unknown_id_is_a_silent_no_op() {
    let mut roster = pooled();
    solo(&mut roster, "Ada", "Lovelace", "3");
    let events = roster.audit_log().len();

    RosterView::mark_has_data(&mut roster, &EntryId("missing".to_owned()));

    assert_eq!(roster.audit_log().len(), events);
}

// ---------------------------------------------------------- set_entry_dnf

#[test]
fn test_view_set_entry_dnf_sets_the_entry_status() {
    let mut roster = pooled();
    let id = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    RosterView::set_entry_dnf(&mut roster, &id, "1", false, "bike broke");

    assert_eq!(
        roster
            .entry_by_key(&id.0)
            .expect("the entry is known")
            .status,
        EntryStatus::Dnf
    );
}

#[test]
fn test_view_set_entry_dnf_makes_the_snapshot_report_dnf() {
    let mut roster = pooled();
    let id = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    RosterView::set_entry_dnf(&mut roster, &id, "1", false, "bike broke");

    assert_eq!(RosterView::entries(&roster)[0].is_dnf, true);
}

#[test]
fn test_view_set_entry_dnf_accepts_a_retired_entry() {
    let mut roster = pooled();
    let id = dissolved_solo(&mut roster);

    RosterView::set_entry_dnf(&mut roster, &id, "1", false, "bike broke");

    assert_eq!(
        RosterView::entry(&roster, &id).map(|snap| snap.is_dnf),
        Some(true)
    );
}

#[test]
fn test_view_set_entry_dnf_ignores_the_rider_flag() {
    let mut roster = pooled();
    let id = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    RosterView::set_entry_dnf(&mut roster, &id, "1", true, "bike broke");

    assert_eq!(
        roster
            .entry_by_key(&id.0)
            .expect("the entry is known")
            .status,
        EntryStatus::Dnf
    );
}

#[test]
fn test_view_set_entry_dnf_given_an_unknown_id_is_a_silent_no_op() {
    let mut roster = pooled();
    let id = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let events = roster.audit_log().len();

    RosterView::set_entry_dnf(&mut roster, &EntryId("missing".to_owned()), "1", false, "x");

    assert_eq!(roster.audit_log().len(), events);
    assert_eq!(
        roster
            .entry_by_key(&id.0)
            .expect("the entry is known")
            .status,
        EntryStatus::Active
    );
}

#[test]
fn test_view_set_entry_dnf_audits_the_plate() {
    let mut roster = pooled();
    let id = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    RosterView::set_entry_dnf(&mut roster, &id, "2", false, "bike broke");

    let event = roster.audit_log().last().expect("the mark is audited");
    assert_eq!(event.action, "set_entry_dnf");
    assert_eq!(event.payload, serde_json::json!({"plate": "2"}));
}

// ---------------------------------------------------------------- moves

#[test]
fn test_view_move_rider_accepts_and_mutates() {
    let mut roster = pooled();
    let source = team(
        &mut roster,
        "Team A",
        &[("Alex", "1"), ("Bo", "2"), ("El", "3")],
    );
    let destination = team(&mut roster, "Team B", &[("Cy", "10"), ("Do", "11")]);

    RosterView::move_rider(&mut roster, "1", &destination, "rider swapped teams")
        .expect("the move is allowed");

    let destination_snapshot =
        RosterView::entry(&roster, &destination).expect("the destination exists");
    let source_snapshot = RosterView::entry(&roster, &source).expect("the source exists");
    assert_eq!(destination_snapshot.riders.len(), 3);
    assert_eq!(destination_snapshot.plate, "1");
    assert_eq!(destination_snapshot.riders[2].plate, Some("1".to_owned()));
    assert_eq!(source_snapshot.riders.len(), 2);
    assert_eq!(source_snapshot.plate, "2");
}

#[test]
fn test_view_move_rider_given_a_non_team_destination_maps_the_roster_error() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let solo_id = solo(&mut roster, "Ada", "Lovelace", "5");

    let result = RosterView::move_rider(&mut roster, "1", &solo_id, "swap");

    assert_eq!(
        result,
        Err(RideError::Roster {
            message: "move_rider's destination must be a team entry".to_owned(),
        })
    );
}

#[test]
fn test_view_move_rider_given_an_unknown_rider_maps_the_roster_error() {
    let mut roster = pooled();
    team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);
    let destination = team(&mut roster, "Team B", &[("Cy", "10"), ("Do", "11")]);

    let result = RosterView::move_rider(&mut roster, "99", &destination, "swap");

    assert_eq!(
        result,
        Err(RideError::Roster {
            message: "rider is not on any entry in this roster".to_owned(),
        })
    );
}

#[test]
fn test_view_extract_rider_to_solo_returns_the_new_entry() {
    let mut roster = pooled();
    let source = team(&mut roster, "Team A", &[("Alex", "1"), ("Bo", "2")]);

    let new_id = RosterView::extract_rider_to_solo(&mut roster, "2", "rider goes solo")
        .expect("the extraction is allowed");

    let new_snapshot = RosterView::entry(&roster, &new_id).expect("the solo entry exists");
    let source_snapshot = RosterView::entry(&roster, &source).expect("the source exists");
    assert_eq!(new_snapshot.display_name, "Bo");
    assert_eq!(new_snapshot.plate, "2");
    assert_eq!(
        (new_snapshot.is_team, new_snapshot.riders.len()),
        (false, 1)
    );
    assert_eq!(source_snapshot.riders.len(), 1);
    assert_eq!(source_snapshot.plate, "1");
}

#[test]
fn test_view_extract_rider_to_solo_given_a_solo_rider_maps_the_roster_error() {
    let mut roster = pooled();
    solo(&mut roster, "Ada", "Lovelace", "1");

    let result = RosterView::extract_rider_to_solo(&mut roster, "1", "rider goes solo");

    assert_eq!(
        result,
        Err(RideError::Roster {
            message: "extract_rider_to_solo requires a rider_pooled team member".to_owned(),
        })
    );
}
