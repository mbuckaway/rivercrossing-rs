// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the app state: the phase and gate projections, the status
//! notice, the settings write and the new-ride seed.

use std::path::PathBuf;

use pretty_assertions::assert_eq;
use rivercrossing_ride::{
    EntryMode, PlateModel, RideConfig, RideConfigParams, RideEngine, SystemClock,
};
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::Rider;
use rivercrossing_store::Store;
use rstest::rstest;

use super::{AppState, ExportPresence, LoadedRide, new_ride_seed};
use crate::ui::action::enable::{GateState, RidePhase};
use crate::ui::dialogs::stack::DialogStack;

/// The ride name every fixture uses.
const RIDE_NAME: &str = "GORBA EPIC 2026";

/// The plate that the fixture's solo entry carries.
const PLATE: &str = "101";

/// The seed every fixture ride stores.
const SEED: u64 = 7;

// ------------------------------------------------------------- fixtures

/// Return the fixture ride config; its setup clears the start minimum.
fn fixture_config() -> Result<RideConfig, String> {
    RideConfig::new(RideConfigParams {
        name: RIDE_NAME.to_owned(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        ..RideConfigParams::default()
    })
    .map_err(|error| error.to_string())
}

/// Return a roster that holds one solo entry on `PLATE`.
fn fixture_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster
        .create_solo_entry(
            Rider {
                first_name: "Ada".to_owned(),
                last_name: "Lovelace".to_owned(),
                ..Rider::default()
            },
            PLATE,
        )
        .map_err(|error| error.to_string())?;
    Ok(roster)
}

/// Return an empty app state over the default settings.
fn empty_state() -> AppState {
    AppState {
        settings: rivercrossing_store::default_settings(),
        settings_path: PathBuf::new(),
        ride: None,
        dialogs: DialogStack::default(),
        notice: None,
        exports: ExportPresence {
            html: false,
            poster_html: false,
            pdf: false,
        },
        audit_row_count: 0,
    }
}

/// Move *engine* through the lifecycle until it reaches *phase*.
fn advance_to(
    engine: &mut RideEngine<Roster, SystemClock>,
    phase: RidePhase,
) -> Result<(), String> {
    if phase == RidePhase::Draft {
        return Ok(());
    }
    engine.start(None).map_err(|error| error.to_string())?;
    if phase == RidePhase::Running {
        return Ok(());
    }
    engine.stop().map_err(|error| error.to_string())?;
    if phase == RidePhase::RunningStopped {
        return Ok(());
    }
    engine.finish(&[]).map_err(|error| error.to_string())?;
    if phase == RidePhase::Finished {
        return Ok(());
    }
    engine.reopen().map_err(|error| error.to_string())?;
    Ok(())
}

/// Build a loaded ride in *phase*, or `None` for [`RidePhase::None`].
///
/// The engine comes from a real temporary database, because the ride crate
/// builds an engine only over its seeded shoe, which the store owns.
fn loaded_ride(phase: RidePhase) -> Result<Option<LoadedRide>, String> {
    if phase == RidePhase::None {
        return Ok(None);
    }
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let config = fixture_config()?;
    let ride_id = store
        .create_ride(&config, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, &fixture_roster()?)
        .map_err(|error| error.to_string())?;
    let mut engine = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;
    advance_to(&mut engine, phase)?;
    let roster = store
        .roster_for(ride_id)
        .map_err(|error| error.to_string())?;
    Ok(Some(LoadedRide {
        ride_id,
        config: engine.config().clone(),
        roster,
        engine,
    }))
}

// ----------------------------------------------------------------- tests

#[rstest]
#[case(RidePhase::None)]
#[case(RidePhase::Draft)]
#[case(RidePhase::Running)]
#[case(RidePhase::RunningStopped)]
#[case(RidePhase::Finished)]
#[case(RidePhase::Reopened)]
fn test_phase_maps_status_and_stopped_to_the_action_gate(
    #[case] expected: RidePhase,
) -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(expected)?;

    // Act
    let phase = state.phase();

    // Assert
    assert_eq!(phase, expected);
    Ok(())
}

#[test]
fn test_gate_reports_teams_crossings_audit_and_show_times() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    let mut ride = loaded_ride(RidePhase::Running)?
        .ok_or_else(|| "the running ride is not built".to_owned())?;
    assert_eq!(ride.ride_id, 1);
    assert_eq!(ride.roster.entries().len(), 1);
    let _ = ride.engine.record_crossing(PLATE, None);
    state.ride = Some(ride);
    state.audit_row_count = 1;
    state.exports = ExportPresence {
        html: true,
        poster_html: true,
        pdf: true,
    };
    state.settings.publish_show_times = true;

    // Act
    let gate = state.gate();

    // Assert
    assert_eq!(
        gate,
        GateState {
            phase: RidePhase::Running,
            teams_enabled: true,
            has_crossing: true,
            has_audit_row: true,
            html_export: true,
            poster_export: true,
            pdf_export: true,
            show_times: true,
        }
    );
    Ok(())
}

#[test]
fn test_gate_reports_a_closed_gate_on_a_fresh_state() {
    // Arrange
    let state = empty_state();

    // Act
    let gate = state.gate();

    // Assert
    assert_eq!(gate, GateState::default());
    assert_eq!(state.dialogs.top(), None);
    assert_eq!(state.audit_row_count, 0);
}

#[rstest]
#[case("saved", Some("saved".to_owned()))]
#[case("", None)]
fn test_note_sets_and_clears_the_notice(#[case] message: &str, #[case] expected: Option<String>) {
    // Arrange
    let mut state = empty_state();

    // Act
    state.note(message);

    // Assert
    assert_eq!(state.notice, expected);
}

#[test]
fn test_save_settings_writes_the_state_file() -> Result<(), String> {
    // Arrange
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("settings.json");
    let mut state = empty_state();
    state.settings_path = path.clone();
    state.settings.wp_url = "https://example.test".to_owned();

    // Act
    state.save_settings().map_err(|error| error.to_string())?;

    // Assert
    let (stored, warnings) = rivercrossing_store::load_settings(&path);
    assert_eq!(stored.wp_url, "https://example.test");
    assert_eq!(warnings.len(), 0);
    Ok(())
}

#[test]
fn test_a_new_ride_seed_is_63_bits() {
    // Act
    let first = new_ride_seed();
    let second = new_ride_seed();

    // Assert
    assert_eq!(first >> 63, 0);
    assert_eq!(second >> 63, 0);
    assert_ne!(first, second);
}
