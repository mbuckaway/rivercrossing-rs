// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the rider editor's decisions, the live-move routing and the
//! CSV preview/import/export wiring.
//!
//! The rules and the copy follow the Python oracle's
//! `ui/presenters/riders.py`; the families are ported by rule.

use std::path::PathBuf;

use pretty_assertions::assert_eq;
use rivercrossing_csvio as csvio;
use rivercrossing_ride::{
    EntryMode, EventAction, PlateModel, RideConfig, RideConfigParams, RideEngine, SystemClock,
};
use rivercrossing_roster::enums::{EntryType, RideStatus};
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::{Entry, EntryId, Rider};
use rivercrossing_store::Store;
use rstest::rstest;

use super::{
    CsvPreviewModel, RiderFormValues, RiderRow, TeamChoice, add_rider, commit_live_move,
    csv_export, csv_import, csv_preview, delete_message, delete_rider, edit_rider, filter_rows,
    is_live_move, rows,
};

/// The seat seed every fixture ride stores.
const SEED: u64 = 7;

/// The ride name every engine fixture uses.
const RIDE_NAME: &str = "GORBA EPIC 2026";

/// The canonical CSV header.
const HEADER: &str = "FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\n";

// ---------------------------------------------------------------- helpers

/// Return an empty mixed ride that pools rider plates.
fn pooled(max_team_size: u32) -> Result<Roster, String> {
    Roster::new(
        EntryMode::Mixed,
        max_team_size,
        PlateModel::RiderPooled,
        Some(SEED),
    )
    .map_err(|error| error.to_string())
}

/// Return an empty mixed ride that plates whole teams.
fn relay() -> Result<Roster, String> {
    Roster::new(EntryMode::Mixed, 4, PlateModel::TeamRelay, Some(SEED))
        .map_err(|error| error.to_string())
}

/// Add one solo entry and return its id.
#[expect(
    clippy::too_many_arguments,
    reason = "one fixture call spells the rider's name and plate"
)]
fn add_solo(roster: &mut Roster, first: &str, last: &str, plate: &str) -> Result<EntryId, String> {
    roster
        .create_solo_entry(
            Rider {
                first_name: first.to_owned(),
                last_name: last.to_owned(),
                ..Rider::default()
            },
            plate,
        )
        .map_err(|error| error.to_string())
}

/// Add one pooled team from its `(first, last, plate)` members.
fn add_pooled_team(
    roster: &mut Roster,
    name: &str,
    members: &[(&str, &str, &str)],
) -> Result<EntryId, String> {
    let riders = members
        .iter()
        .map(|(first, last, plate)| Rider {
            first_name: (*first).to_owned(),
            last_name: (*last).to_owned(),
            plate: Some((*plate).to_owned()),
            ..Rider::default()
        })
        .collect();
    roster
        .create_team_entry(TeamFields {
            display_name: name,
            riders,
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())
}

/// Add one relay team on *plate* from its `(first, last)` members.
#[expect(
    clippy::too_many_arguments,
    reason = "one fixture call spells the team, its plate and its members"
)]
fn add_relay_team(
    roster: &mut Roster,
    name: &str,
    plate: &str,
    members: &[(&str, &str)],
) -> Result<EntryId, String> {
    let riders = members
        .iter()
        .map(|(first, last)| Rider {
            first_name: (*first).to_owned(),
            last_name: (*last).to_owned(),
            ..Rider::default()
        })
        .collect();
    roster
        .create_team_entry(TeamFields {
            display_name: name,
            riders,
            plate: Some(plate),
            logo_card: None,
        })
        .map_err(|error| error.to_string())
}

/// Return the live entry that *id* names.
fn entry<'a>(roster: &'a Roster, id: &EntryId) -> Result<&'a Entry, String> {
    roster
        .entry_by_key(&id.0)
        .ok_or_else(|| format!("no entry {}", id.0))
}

/// Build a rider form with no sex.
#[expect(
    clippy::too_many_arguments,
    reason = "one fixture call spells the plate, both names and the team choice"
)]
fn form(plate: &str, first: &str, last: &str, team: TeamChoice) -> RiderFormValues {
    RiderFormValues {
        plate: plate.to_owned(),
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        team,
        sex: None,
    }
}

/// Return a valid engine config under *mode* and *model*.
fn config(entry_mode: EntryMode, plate_model: PlateModel) -> Result<RideConfig, String> {
    RideConfig::new(RideConfigParams {
        name: RIDE_NAME.to_owned(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        entry_mode,
        plate_model,
        ..RideConfigParams::default()
    })
    .map_err(|error| error.to_string())
}

/// Build a DRAFT engine over *roster* through a real temporary store.
fn draft_engine(
    roster: &Roster,
    config: &RideConfig,
) -> Result<RideEngine<Roster, SystemClock>, String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(config, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, roster)
        .map_err(|error| error.to_string())?;
    store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())
}

/// Build a RUNNING engine over *roster*.
fn running_engine(
    roster: &Roster,
    config: &RideConfig,
) -> Result<RideEngine<Roster, SystemClock>, String> {
    let mut engine = draft_engine(roster, config)?;
    engine.start(None).map_err(|error| error.to_string())?;
    Ok(engine)
}

/// Build a stopped RUNNING engine over *roster*.
fn stopped_engine(
    roster: &Roster,
    config: &RideConfig,
) -> Result<RideEngine<Roster, SystemClock>, String> {
    let mut engine = running_engine(roster, config)?;
    engine.stop().map_err(|error| error.to_string())?;
    Ok(engine)
}

/// Build a REOPENED engine over *roster*.
fn reopened_engine(
    roster: &Roster,
    config: &RideConfig,
) -> Result<RideEngine<Roster, SystemClock>, String> {
    let mut engine = running_engine(roster, config)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;
    engine.reopen().map_err(|error| error.to_string())?;
    Ok(engine)
}

/// Write *body* under *dir* and return the path.
fn write_csv(dir: &tempfile::TempDir, name: &str, body: &str) -> Result<PathBuf, String> {
    let path = dir.path().join(name);
    std::fs::write(&path, format!("{HEADER}{body}")).map_err(|error| error.to_string())?;
    Ok(path)
}

/// Return the roster that every CSV preview fixture previews against.
fn csv_roster() -> Result<Roster, String> {
    pooled(4)
}

// -------------------------------------------------------------- add rider

#[test]
fn test_add_rider_given_a_solo_choice_creates_canonicalised_solo_entry() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let values = RiderFormValues {
        plate: "101".to_owned(),
        first_name: "ada".to_owned(),
        last_name: "LOVELACE".to_owned(),
        team: TeamChoice::Solo,
        sex: Some("F".to_owned()),
    };

    // Act
    let id = add_rider(&mut roster, &values)?;

    // Assert
    let created = entry(&roster, &id)?;
    assert_eq!(created.display_name, "Ada Lovelace");
    assert_eq!(created.kind, EntryType::Solo);
    assert_eq!(created.plate, "101");
    let rider = created
        .riders
        .first()
        .ok_or("the solo entry has no rider")?;
    assert_eq!(rider.first_name, "Ada");
    assert_eq!(rider.last_name, "Lovelace");
    assert_eq!(rider.sex.as_deref(), Some("F"));
    assert_eq!(rider.plate.as_deref(), Some("101"));
    Ok(())
}

#[test]
fn test_add_rider_given_a_team_choice_folds_the_rider_onto_the_team() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let team = add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Ada", "Lovelace", "201"), ("Alan", "Turing", "202")],
    )?;
    let values = form(
        "203",
        "Grace",
        "Hopper",
        TeamChoice::Team("Alpha".to_owned()),
    );

    // Act
    let id = add_rider(&mut roster, &values)?;

    // Assert
    assert_eq!(id, team);
    let target = entry(&roster, &team)?;
    assert_eq!(target.team_size(), 3);
    assert_eq!(
        target.riders.last().map(Rider::full_name),
        Some("Grace Hopper".to_owned())
    );
    Ok(())
}

#[rstest]
#[case("", "Lovelace")]
#[case("Ada", "   ")]
fn test_add_rider_given_a_blank_name_is_refused(
    #[case] first: &str,
    #[case] last: &str,
) -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let values = form("101", first, last, TeamChoice::Solo);

    // Act
    let result = add_rider(&mut roster, &values);

    // Assert
    assert_eq!(
        result,
        Err("First name and last name are required".to_owned())
    );
    assert_eq!(roster.entries().len(), 0);
    Ok(())
}

#[test]
fn test_add_rider_given_a_duplicate_plate_surfaces_the_roster_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    let values = form("101", "Grace", "Hopper", TeamChoice::Solo);

    // Act
    let result = add_rider(&mut roster, &values);

    // Assert
    assert_eq!(result, Err("plate '101' is already in use".to_owned()));
    assert_eq!(roster.entries().len(), 1);
    Ok(())
}

#[test]
fn test_add_rider_given_a_full_team_surfaces_the_team_size_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(2)?;
    add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Ada", "Lovelace", "201"), ("Alan", "Turing", "202")],
    )?;
    let values = form(
        "203",
        "Grace",
        "Hopper",
        TeamChoice::Team("Alpha".to_owned()),
    );

    // Act
    let result = add_rider(&mut roster, &values);

    // Assert
    assert_eq!(result, Err("team size must be at most 2, got 3".to_owned()));
    Ok(())
}

#[test]
fn test_add_rider_given_an_unknown_team_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let values = form(
        "101",
        "Ada",
        "Lovelace",
        TeamChoice::Team("Ghost".to_owned()),
    );

    // Act
    let result = add_rider(&mut roster, &values);

    // Assert
    assert_eq!(result, Err("no team named \"Ghost\"".to_owned()));
    assert_eq!(roster.entries().len(), 0);
    Ok(())
}

// ------------------------------------------------------------- edit rider

#[test]
fn test_edit_rider_renames_and_replates_a_solo_entry() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    let values = form("150", "grace", "HOPPER", TeamChoice::Solo);

    // Act
    edit_rider(&mut roster, &id, 0, &values)?;

    // Assert
    let edited = entry(&roster, &id)?;
    assert_eq!(edited.plate, "150");
    assert_eq!(edited.display_name, "Grace Hopper");
    assert_eq!(
        edited.riders.first().map(Rider::full_name),
        Some("Grace Hopper".to_owned())
    );
    assert_eq!(
        edited.riders.first().and_then(|rider| rider.plate.clone()),
        Some("150".to_owned())
    );
    Ok(())
}

#[rstest]
#[case("McDonald", "McDonald")]
#[case("VAN DER BERG", "Van der berg")]
#[case(" o'brien ", "O'brien")]
fn test_edit_rider_given_a_deliberate_surname_keeps_its_case(
    #[case] typed: &str,
    #[case] expected: &str,
) -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    let values = form("101", "Ada", typed, TeamChoice::Solo);

    // Act
    edit_rider(&mut roster, &id, 0, &values)?;

    // Assert
    let edited = entry(&roster, &id)?;
    assert_eq!(
        edited.riders.first().map(|rider| rider.last_name.as_str()),
        Some(expected)
    );
    Ok(())
}

#[test]
fn test_edit_rider_given_a_blank_plate_surfaces_the_shape_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    let values = form(" ", "Ada", "Lovelace", TeamChoice::Solo);

    // Act
    let result = edit_rider(&mut roster, &id, 0, &values);

    // Assert
    assert_eq!(result, Err("plate ' ' must not be empty".to_owned()));
    Ok(())
}

#[test]
fn test_edit_rider_given_a_duplicate_plate_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    add_solo(&mut roster, "Alan", "Turing", "102")?;
    let values = form("102", "Ada", "Lovelace", TeamChoice::Solo);

    // Act
    let result = edit_rider(&mut roster, &id, 0, &values);

    // Assert
    assert_eq!(result, Err("plate '102' is already in use".to_owned()));
    Ok(())
}

#[test]
fn test_edit_rider_moves_a_pooled_rider_onto_another_team() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let alpha = add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Ada", "Lovelace", "101"), ("Alan", "Turing", "102")],
    )?;
    let bravo = add_pooled_team(
        &mut roster,
        "Bravo",
        &[("Grace", "Hopper", "103"), ("Edsger", "Dijkstra", "104")],
    )?;
    let values = form("101", "Ada", "Byron", TeamChoice::Team("Bravo".to_owned()));

    // Act
    edit_rider(&mut roster, &alpha, 0, &values)?;

    // Assert
    assert_eq!(entry(&roster, &alpha)?.team_size(), 1);
    let destination = entry(&roster, &bravo)?;
    assert_eq!(destination.team_size(), 3);
    assert_eq!(
        destination.riders.last().map(Rider::full_name),
        Some("Ada Byron".to_owned())
    );
    Ok(())
}

#[test]
fn test_edit_rider_moves_a_solo_rider_onto_a_team() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let solo = add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    let alpha = add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Grace", "Hopper", "201"), ("Alan", "Turing", "202")],
    )?;
    let values = form("101", "Ada", "Byron", TeamChoice::Team("Alpha".to_owned()));

    // Act
    edit_rider(&mut roster, &solo, 0, &values)?;

    // Assert
    assert_eq!(roster.entry_by_key(&solo.0).map(|entry| entry.kind), None);
    let destination = entry(&roster, &alpha)?;
    assert_eq!(destination.team_size(), 3);
    assert_eq!(
        destination.riders.last().map(Rider::full_name),
        Some("Ada Byron".to_owned())
    );
    Ok(())
}

#[test]
fn test_edit_rider_extracts_a_team_member_to_solo() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let alpha = add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Ada", "Lovelace", "101"), ("Alan", "Turing", "102")],
    )?;
    let values = form("102", "Alan", "Turing", TeamChoice::Solo);

    // Act
    edit_rider(&mut roster, &alpha, 1, &values)?;

    // Assert
    assert_eq!(entry(&roster, &alpha)?.team_size(), 1);
    let extracted = roster
        .entries()
        .iter()
        .find(|entry| entry.kind == EntryType::Solo)
        .ok_or("the extraction created no solo entry")?;
    assert_eq!(extracted.plate, "102");
    assert_eq!(
        extracted.riders.first().map(Rider::full_name),
        Some("Alan Turing".to_owned())
    );
    Ok(())
}

#[test]
fn test_edit_rider_given_a_full_destination_team_keeps_the_solo_entry() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(2)?;
    let solo = add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    add_pooled_team(
        &mut roster,
        "Bravo",
        &[("Grace", "Hopper", "201"), ("Alan", "Turing", "202")],
    )?;
    let values = form(
        "101",
        "Ada",
        "Lovelace",
        TeamChoice::Team("Bravo".to_owned()),
    );

    // Act
    let result = edit_rider(&mut roster, &solo, 0, &values);

    // Assert
    assert_eq!(result, Err("team size must be at most 2, got 3".to_owned()));
    assert_eq!(
        roster.entry_by_key(&solo.0).map(|entry| entry.kind),
        Some(EntryType::Solo)
    );
    Ok(())
}

#[test]
fn test_edit_rider_on_a_relay_ride_replates_the_team() -> Result<(), String> {
    // Arrange
    let mut roster = relay()?;
    let alpha = add_relay_team(
        &mut roster,
        "Alpha",
        "1",
        &[("Ada", "Lovelace"), ("Alan", "Turing")],
    )?;
    let values = form("9", "Ada", "Lovelace", TeamChoice::Team("Alpha".to_owned()));

    // Act
    edit_rider(&mut roster, &alpha, 0, &values)?;

    // Assert
    assert_eq!(entry(&roster, &alpha)?.plate, "9");
    Ok(())
}

#[test]
fn test_edit_rider_given_a_relay_team_change_keeps_the_destination_plate() -> Result<(), String> {
    // Arrange
    let mut roster = relay()?;
    let alpha = add_relay_team(
        &mut roster,
        "Alpha",
        "1",
        &[("Ada", "Lovelace"), ("Alan", "Turing")],
    )?;
    let bravo = add_relay_team(
        &mut roster,
        "Bravo",
        "2",
        &[("Grace", "Hopper"), ("Edsger", "Dijkstra")],
    )?;
    let values = form("2", "Ada", "Byron", TeamChoice::Team("Bravo".to_owned()));

    // Act
    edit_rider(&mut roster, &alpha, 0, &values)?;

    // Assert
    assert_eq!(entry(&roster, &bravo)?.plate, "2");
    assert_eq!(entry(&roster, &alpha)?.plate, "1");
    assert_eq!(entry(&roster, &bravo)?.team_size(), 3);
    Ok(())
}

#[test]
fn test_edit_rider_given_an_unknown_entry_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let missing = EntryId("missing".to_owned());
    let values = form("101", "Ada", "Lovelace", TeamChoice::Solo);

    // Act
    let result = edit_rider(&mut roster, &missing, 0, &values);

    // Assert
    assert_eq!(
        result,
        Err("entry missing is not a member of this roster".to_owned())
    );
    Ok(())
}

#[test]
fn test_edit_rider_given_an_unknown_team_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    let values = form(
        "101",
        "Ada",
        "Lovelace",
        TeamChoice::Team("Ghost".to_owned()),
    );

    // Act
    let result = edit_rider(&mut roster, &id, 0, &values);

    // Assert
    assert_eq!(result, Err("no team named \"Ghost\"".to_owned()));
    Ok(())
}

#[test]
fn test_edit_rider_given_a_rider_index_out_of_range_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    let values = form("101", "Ada", "Lovelace", TeamChoice::Solo);

    // Act
    let result = edit_rider(&mut roster, &id, 9, &values);

    // Assert
    assert_eq!(
        result,
        Err(format!("entry {} has no rider at index 9", id.0))
    );
    Ok(())
}

#[test]
fn test_edit_rider_on_a_locked_ride_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    roster.set_status(RideStatus::Running);
    let values = form("150", "Ada", "Lovelace", TeamChoice::Solo);

    // Act
    let result = edit_rider(&mut roster, &id, 0, &values);

    // Assert
    assert_eq!(
        result,
        Err("plates cannot be changed once the ride is running".to_owned())
    );
    Ok(())
}

// ---------------------------------------------------------- is live move

#[test]
fn test_is_live_move_given_a_team_change_on_a_running_engine_is_true() -> Result<(), String> {
    // Arrange
    let roster = solo_only_roster()?;
    let engine = running_engine(&roster, &config(EntryMode::Mixed, PlateModel::RiderPooled)?)?;
    let values = form(
        "101",
        "Ada",
        "Lovelace",
        TeamChoice::Team("Alpha".to_owned()),
    );

    // Assert
    assert!(is_live_move(&engine, &values, None));
    Ok(())
}

#[test]
fn test_is_live_move_given_a_team_change_on_a_reopened_engine_is_true() -> Result<(), String> {
    // Arrange
    let roster = solo_only_roster()?;
    let engine = reopened_engine(&roster, &config(EntryMode::Mixed, PlateModel::RiderPooled)?)?;
    let values = form(
        "101",
        "Ada",
        "Lovelace",
        TeamChoice::Team("Alpha".to_owned()),
    );

    // Assert
    assert!(is_live_move(&engine, &values, None));
    Ok(())
}

#[test]
fn test_is_live_move_given_an_unchanged_team_is_false() -> Result<(), String> {
    // Arrange
    let roster = solo_only_roster()?;
    let engine = running_engine(&roster, &config(EntryMode::Mixed, PlateModel::RiderPooled)?)?;
    let values = form(
        "101",
        "Ada",
        "Lovelace",
        TeamChoice::Team("Alpha".to_owned()),
    );

    // Assert
    assert!(!is_live_move(&engine, &values, Some("Alpha")));
    Ok(())
}

#[test]
fn test_is_live_move_given_a_draft_engine_is_false() -> Result<(), String> {
    // Arrange
    let roster = solo_only_roster()?;
    let engine = draft_engine(&roster, &config(EntryMode::Mixed, PlateModel::RiderPooled)?)?;
    let values = form(
        "101",
        "Ada",
        "Lovelace",
        TeamChoice::Team("Alpha".to_owned()),
    );

    // Assert
    assert!(!is_live_move(&engine, &values, None));
    Ok(())
}

/// Return a one-solo roster: the move tests only need a startable ride.
fn solo_only_roster() -> Result<Roster, String> {
    let mut roster = pooled(4)?;
    add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    Ok(roster)
}

/// Return a two-team roster with one pooled rider on each.
fn two_team_roster() -> Result<Roster, String> {
    let mut roster = pooled(4)?;
    add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Ada", "Lovelace", "101"), ("Alan", "Turing", "102")],
    )?;
    add_pooled_team(
        &mut roster,
        "Bravo",
        &[("Grace", "Hopper", "103"), ("Edsger", "Dijkstra", "104")],
    )?;
    Ok(roster)
}

/// Return a pooled config for the two-team roster.
fn two_team_config() -> Result<RideConfig, String> {
    config(EntryMode::Mixed, PlateModel::RiderPooled)
}

// -------------------------------------------------------- commit live move

#[test]
fn test_commit_live_move_given_a_running_engine_refuses_to_stop_first() -> Result<(), String> {
    // Arrange
    let roster = two_team_roster()?;
    let mut engine = running_engine(&roster, &two_team_config()?)?;
    let values = form(
        "101",
        "Ada",
        "Lovelace",
        TeamChoice::Team("Bravo".to_owned()),
    );

    // Act
    let result = commit_live_move(&mut engine, "101", &values);

    // Assert
    assert_eq!(result, Err("Stop the ride first".to_owned()));
    assert_eq!(
        engine.events().last().map(|event| event.action),
        Some(EventAction::Start)
    );
    Ok(())
}

#[test]
fn test_commit_live_move_given_a_stopped_engine_moves_the_rider() -> Result<(), String> {
    // Arrange
    let roster = two_team_roster()?;
    let mut engine = stopped_engine(&roster, &two_team_config()?)?;
    let values = form(
        "101",
        "Ada",
        "Lovelace",
        TeamChoice::Team("Bravo".to_owned()),
    );

    // Act
    let result = commit_live_move(&mut engine, "101", &values);

    // Assert
    assert_eq!(result, Ok("Rider Editor: moved rider to Bravo".to_owned()));
    assert_eq!(
        engine.events().last().map(|event| event.action),
        Some(EventAction::MoveRider)
    );
    Ok(())
}

#[test]
fn test_commit_live_move_given_the_solo_choice_extracts_the_rider() -> Result<(), String> {
    // Arrange
    let roster = two_team_roster()?;
    let mut engine = stopped_engine(&roster, &two_team_config()?)?;
    let values = form("101", "Ada", "Lovelace", TeamChoice::Solo);

    // Act
    let result = commit_live_move(&mut engine, "101", &values);

    // Assert
    assert_eq!(result, Ok("Rider Editor: moved rider to solo".to_owned()));
    assert_eq!(
        engine.events().last().map(|event| event.action),
        Some(EventAction::ExtractRiderToSolo)
    );
    Ok(())
}

#[test]
fn test_commit_live_move_given_an_unknown_team_surfaces_the_engine_refusal() -> Result<(), String> {
    // Arrange
    let roster = two_team_roster()?;
    let mut engine = stopped_engine(&roster, &two_team_config()?)?;
    let values = form(
        "101",
        "Ada",
        "Lovelace",
        TeamChoice::Team("Ghost".to_owned()),
    );

    // Act
    let result = commit_live_move(&mut engine, "101", &values);

    // Assert
    assert_eq!(result, Err("unknown team: Ghost".to_owned()));
    Ok(())
}

// ------------------------------------------------------------------- rows

#[test]
fn test_rows_given_a_solo_and_a_team_lists_every_rider_with_its_cells() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Grace", "Hopper", "201"), ("Alan", "Turing", "202")],
    )?;

    // Act
    let rendered = rows(&roster);

    // Assert
    assert_eq!(
        rendered,
        vec![
            RiderRow {
                plate: "101".to_owned(),
                name: "Ada Lovelace".to_owned(),
                team: "solo".to_owned(),
            },
            RiderRow {
                plate: "201".to_owned(),
                name: "Grace Hopper".to_owned(),
                team: "Alpha".to_owned(),
            },
            RiderRow {
                plate: "202".to_owned(),
                name: "Alan Turing".to_owned(),
                team: "Alpha".to_owned(),
            },
        ]
    );
    Ok(())
}

#[test]
fn test_rows_on_a_relay_ride_shows_the_entry_plate_for_every_rider() -> Result<(), String> {
    // Arrange
    let mut roster = relay()?;
    add_relay_team(
        &mut roster,
        "Alpha",
        "7",
        &[("Ada", "Lovelace"), ("Alan", "Turing")],
    )?;

    // Act
    let rendered = rows(&roster);

    // Assert
    assert_eq!(
        rendered
            .iter()
            .map(|row| row.plate.as_str())
            .collect::<Vec<_>>(),
        vec!["7", "7"]
    );
    Ok(())
}

#[rstest]
#[case("", 3)]
#[case("   ", 3)]
#[case("solo", 1)]
#[case("ALPHA", 2)]
#[case("201", 1)]
#[case("zz", 0)]
fn test_filter_rows_matches_plate_name_and_team_case_insensitively(
    #[case] search: &str,
    #[case] expected: usize,
) -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Grace", "Hopper", "201"), ("Alan", "Turing", "202")],
    )?;
    let rendered = rows(&roster);

    // Act
    let filtered = filter_rows(&rendered, search);

    // Assert
    assert_eq!(filtered.len(), expected);
    Ok(())
}

#[test]
fn test_filter_rows_matches_a_name_substring() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Grace", "Hopper", "201"), ("Alan", "Turing", "202")],
    )?;
    let rendered = rows(&roster);

    // Act
    let filtered = filter_rows(&rendered, "hopper");

    // Assert
    assert_eq!(
        filtered.first().map(|row| row.name.as_str()),
        Some("Grace Hopper")
    );
    Ok(())
}

// ----------------------------------------------------------------- delete

#[test]
fn test_delete_message_names_the_entrys_rider() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    let target = entry(&roster, &id)?;

    // Act
    let message = delete_message(target);

    // Assert
    assert_eq!(message, "Delete \"Ada Lovelace\" from this ride?");
    Ok(())
}

#[test]
fn test_delete_message_given_a_multi_rider_team_names_the_team() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Grace", "Hopper", "201"), ("Alan", "Turing", "202")],
    )?;
    let target = entry(&roster, &id)?;

    // Act
    let message = delete_message(target);

    // Assert
    assert_eq!(message, "Delete \"Alpha\" from this ride?");
    Ok(())
}

#[test]
fn test_delete_rider_removes_a_solo_entry() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_solo(&mut roster, "Ada", "Lovelace", "101")?;

    // Act
    delete_rider(&mut roster, &id, 0)?;

    // Assert
    assert_eq!(roster.entries().len(), 0);
    Ok(())
}

#[test]
fn test_delete_rider_removes_one_team_member() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Grace", "Hopper", "201"), ("Alan", "Turing", "202")],
    )?;

    // Act
    delete_rider(&mut roster, &id, 0)?;

    // Assert
    let team = entry(&roster, &id)?;
    assert_eq!(team.team_size(), 1);
    assert_eq!(
        team.riders.first().map(Rider::full_name),
        Some("Alan Turing".to_owned())
    );
    Ok(())
}

#[test]
fn test_delete_rider_given_a_has_data_entry_surfaces_the_lock() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    roster
        .mark_has_data(&id)
        .map_err(|error| error.to_string())?;

    // Act
    let result = delete_rider(&mut roster, &id, 0);

    // Assert
    assert_eq!(
        result,
        Err("entry has recorded data; DNF or void it instead of deleting".to_owned())
    );
    Ok(())
}

#[test]
fn test_delete_rider_given_an_unknown_entry_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let missing = EntryId("missing".to_owned());

    // Act
    let result = delete_rider(&mut roster, &missing, 0);

    // Assert
    assert_eq!(
        result,
        Err("entry missing is not a member of this roster".to_owned())
    );
    Ok(())
}

#[test]
fn test_delete_rider_given_an_index_past_the_team_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Grace", "Hopper", "201"), ("Alan", "Turing", "202")],
    )?;

    // Act
    let result = delete_rider(&mut roster, &id, 9);

    // Assert
    assert_eq!(
        result,
        Err("rider is not on any entry in this roster".to_owned())
    );
    Ok(())
}

// -------------------------------------------------------------------- csv

#[test]
fn test_csv_import_options_default_to_off() {
    // Arrange
    // (the options are the model's own default)

    // Act
    let options = csvio::ImportOptions::default();

    // Assert
    assert_eq!(
        options,
        csvio::ImportOptions {
            map_unknown_sex_to_male: false,
            convert_teams_of_one_to_solo: false,
        }
    );
}

#[test]
fn test_csv_preview_summarises_the_file_and_enables_import() -> Result<(), String> {
    // Arrange
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = write_csv(
        &dir,
        "entries.csv",
        "Ada,Lovelace,team,Alpha,101,,F\nGrace,Hopper,team,Alpha,102,,F\n",
    )?;
    let roster = csv_roster()?;

    // Act
    let (preview, model) = csv_preview(&path, &roster, csvio::ImportOptions::default())?;

    // Assert
    assert_eq!(preview.riders, 2);
    assert_eq!(preview.teams, 1);
    assert_eq!(
        model,
        CsvPreviewModel {
            summary: "entries.csv → 2 riders · 1 teams · 0 conflicts".to_owned(),
            conflicts: Vec::new(),
            can_import: true,
        }
    );
    Ok(())
}

#[test]
fn test_csv_preview_given_a_conflict_disables_import() -> Result<(), String> {
    // Arrange
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = write_csv(
        &dir,
        "entries.csv",
        "Ada,Lovelace,solo,,101,,\nGrace,Hopper,solo,,101,,\n",
    )?;
    let roster = csv_roster()?;

    // Act
    let (_, model) = csv_preview(&path, &roster, csvio::ImportOptions::default())?;

    // Assert
    assert_eq!(
        model.summary,
        "entries.csv → 2 riders · 0 teams · 1 conflicts".to_owned()
    );
    assert_eq!(model.conflicts, vec!["duplicate plate 101".to_owned()]);
    assert!(!model.can_import);
    Ok(())
}

#[test]
fn test_csv_preview_given_warnings_appends_the_warning_count() -> Result<(), String> {
    // Arrange
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = write_csv(&dir, "entries.csv", "Ada,Lovelace,team,Wolves,5,,\n")?;
    let roster = csv_roster()?;

    // Act
    let (_, model) = csv_preview(&path, &roster, csvio::ImportOptions::default())?;

    // Assert
    assert_eq!(
        model.summary,
        "entries.csv → 1 riders · 1 teams · 0 conflicts · 1 warnings".to_owned()
    );
    assert!(model.can_import);
    Ok(())
}

#[test]
fn test_csv_preview_given_an_unreadable_path_reports_it() -> Result<(), String> {
    // Arrange
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = dir.path().join("missing.csv");
    let roster = csv_roster()?;

    // Act
    let result = csv_preview(&path, &roster, csvio::ImportOptions::default());

    // Assert
    assert!(result.is_err_and(|message| message.starts_with("Could not read missing.csv: ")));
    Ok(())
}

#[test]
fn test_csv_preview_given_a_path_without_a_file_name_reports_it() -> Result<(), String> {
    // Arrange
    let path = std::path::Path::new("/");
    let roster = csv_roster()?;

    // Act
    let result = csv_preview(path, &roster, csvio::ImportOptions::default());

    // Assert
    assert!(result.is_err_and(|message| message.starts_with("Could not read /: ")));
    Ok(())
}

#[test]
fn test_csv_import_applies_a_conflict_free_preview() -> Result<(), String> {
    // Arrange
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = write_csv(
        &dir,
        "entries.csv",
        "Ada,Lovelace,team,Alpha,101,,F\nGrace,Hopper,team,Alpha,102,,F\n",
    )?;
    let mut roster = csv_roster()?;
    let (preview, _) = csv_preview(&path, &roster, csvio::ImportOptions::default())?;

    // Act
    let report = csv_import(&mut roster, &preview)?;

    // Assert
    assert_eq!(report.inserted, 1);
    let team = roster
        .entries()
        .first()
        .ok_or("the import inserted no entry")?;
    assert_eq!(team.display_name, "Alpha");
    assert_eq!(team.team_size(), 2);
    Ok(())
}

#[test]
fn test_csv_import_given_a_preview_with_conflicts_is_refused() -> Result<(), String> {
    // Arrange
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = write_csv(
        &dir,
        "entries.csv",
        "Ada,Lovelace,solo,,101,,\nGrace,Hopper,solo,,101,,\n",
    )?;
    let mut roster = csv_roster()?;
    let (preview, _) = csv_preview(&path, &roster, csvio::ImportOptions::default())?;

    // Act
    let result = csv_import(&mut roster, &preview);

    // Assert
    assert_eq!(
        result.map(|report| report.inserted),
        Err("import has 1 blocking conflicts".to_owned())
    );
    assert_eq!(roster.entries().len(), 0);
    Ok(())
}

#[test]
fn test_csv_import_given_a_stale_preview_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = write_csv(&dir, "entries.csv", "Ada,Lovelace,solo,,101,,\n")?;
    let mut roster = csv_roster()?;
    let (preview, _) = csv_preview(&path, &roster, csvio::ImportOptions::default())?;
    add_solo(&mut roster, "Grace", "Hopper", "999")?;

    // Act
    let result = csv_import(&mut roster, &preview);

    // Assert
    assert_eq!(
        result.map(|report| report.inserted),
        Err("the preview is stale; preview the file again".to_owned())
    );
    Ok(())
}

#[test]
fn test_csv_export_writes_the_roster_csv() -> Result<(), String> {
    // Arrange
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = dir.path().join("roster.csv");
    let mut roster = csv_roster()?;
    add_solo(&mut roster, "Ada", "Lovelace", "1")?;

    // Act
    csv_export(&roster, &path, None)?;

    // Assert
    let written = std::fs::read(&path).map_err(|error| error.to_string())?;
    let expected: &[u8] = b"FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\r\n\
Ada,Lovelace,solo,,1,,\r\n";
    assert_eq!(written, expected.to_vec());
    Ok(())
}

#[test]
fn test_csv_export_given_an_unwritable_path_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = dir.path().join("no-such-directory").join("roster.csv");
    let roster = csv_roster()?;

    // Act
    let result = csv_export(&roster, &path, None);

    // Assert
    assert!(result.is_err_and(|message| message.starts_with("io: ")));
    Ok(())
}
