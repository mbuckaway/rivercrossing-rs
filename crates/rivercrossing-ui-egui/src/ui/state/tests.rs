// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the app state: the phase and gate projections, the status
//! notice, the settings write, the new-ride seed and the action execution.

use std::path::PathBuf;

use pretty_assertions::assert_eq;
use rivercrossing_ride::{
    EntryMode, PlateModel, RideConfig, RideConfigParams, RideEngine, SystemClock,
};
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::Rider;
use rivercrossing_store::Store;
use rstest::rstest;

use super::{AppState, ExportPresence, LoadedRide, PendingNative, new_ride_seed};
use crate::ui::action::effect::{DispatchError, Effect};
use crate::ui::action::enable::{GateState, RidePhase};
use crate::ui::action::id::{
    ActionId, MI_ALL_CARDS, MI_BACKUP_NOW, MI_FINISH_RIDE, MI_FULL_FIELD, MI_LAPS_BOARD,
    MI_NEW_RIDE, MI_RIDER_EDITOR, MI_SHORTCUTS, MI_SHOW_DNS_RIDERS, MI_SHOW_LAP_TIME,
    MI_SHOW_TIMES, MI_SHOW_TOTAL_TIMES, MI_STANDINGS, MI_TIME_BOARD, MI_UNDO_CROSSING, MI_ZOOM_90,
    MI_ZOOM_100, MI_ZOOM_110, MI_ZOOM_120, MI_ZOOM_130, MI_ZOOM_140, MI_ZOOM_150, WX_ID_ABOUT,
    WX_ID_EXIT,
};
use crate::ui::action::spec::{CheckKind, CommandId, NativeDialog};
use crate::ui::dialogs::stack::{DialogStack, Focus};

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
        exports: ExportPresence::default(),
        audit_row_count: 0,
        pending_native: None,
        pending_command: None,
    }
}

/// Return an empty app state whose settings are written inside a fresh temporary directory.
fn state_with_temp_settings() -> Result<(tempfile::TempDir, AppState), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut state = empty_state();
    state.settings_path = directory.path().join("settings.json");
    Ok((directory, state))
}

/// Return the setting value that the check toggles.
fn setting_value(state: &AppState, check: CheckKind) -> bool {
    match check {
        CheckKind::PublishShowTimes => state.settings.publish_show_times,
        CheckKind::LapsBoard => state.settings.publish_laps_board,
        CheckKind::TimeBoard => state.settings.publish_time_board,
        CheckKind::FullField => state.settings.publish_full_field,
        CheckKind::AllCards => state.settings.publish_all_cards,
        CheckKind::ShowDnsRiders => state.settings.show_dns_riders,
        CheckKind::ShowTotalTimes => state.settings.show_total_times,
        CheckKind::ShowLapTime => state.settings.show_lap_time,
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

// ------------------------------------------------------- action execution

#[test]
fn test_act_opens_the_setup_dialog_at_its_first_field() {
    // Arrange
    let mut state = empty_state();

    // Act
    let result = state.act(MI_NEW_RIDE);

    // Assert
    assert_eq!(result, Ok(Effect::OpenWindow("ride_setup_dlg")));
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("ride_setup_dlg")
    );
    assert_eq!(
        state.dialogs.top().map(|dialog| &dialog.focus),
        Some(&Focus::Field("name_input"))
    );
}

#[test]
fn test_act_refuses_an_action_the_gate_disables() {
    // Arrange
    let mut state = empty_state();

    // Act
    let result = state.act(MI_FINISH_RIDE);

    // Assert
    assert_eq!(result, Err(DispatchError::Disabled(MI_FINISH_RIDE)));
    assert_eq!(state.pending_native, None);
    assert_eq!(state.dialogs.top(), None);
}

#[rstest]
#[case(CheckKind::PublishShowTimes, MI_SHOW_TIMES)]
#[case(CheckKind::LapsBoard, MI_LAPS_BOARD)]
#[case(CheckKind::TimeBoard, MI_TIME_BOARD)]
#[case(CheckKind::FullField, MI_FULL_FIELD)]
#[case(CheckKind::AllCards, MI_ALL_CARDS)]
#[case(CheckKind::ShowDnsRiders, MI_SHOW_DNS_RIDERS)]
#[case(CheckKind::ShowTotalTimes, MI_SHOW_TOTAL_TIMES)]
#[case(CheckKind::ShowLapTime, MI_SHOW_LAP_TIME)]
fn test_act_toggles_each_check_and_saves(
    #[case] check: CheckKind,
    #[case] id: ActionId,
) -> Result<(), String> {
    // Arrange
    let (_directory, mut state) = state_with_temp_settings()?;
    // The §15 gate lets "Fastest-time leaderboard" act only while "Show lap & total times" is on.
    state.settings.publish_show_times = true;
    let before = setting_value(&state, check);

    // Act
    let result = state.act(id);

    // Assert
    assert_eq!(result, Ok(Effect::ToggleCheck(check)));
    assert_eq!(setting_value(&state, check), !before);
    let (stored, warnings) = rivercrossing_store::load_settings(&state.settings_path);
    assert_eq!(stored, state.settings);
    assert_eq!(warnings.len(), 0);
    Ok(())
}

#[rstest]
#[case(MI_ZOOM_90, 90)]
#[case(MI_ZOOM_100, 100)]
#[case(MI_ZOOM_110, 110)]
#[case(MI_ZOOM_120, 120)]
#[case(MI_ZOOM_130, 130)]
#[case(MI_ZOOM_140, 140)]
#[case(MI_ZOOM_150, 150)]
fn test_act_sets_each_zoom_rung(#[case] id: ActionId, #[case] percent: u16) -> Result<(), String> {
    // Arrange
    let (_directory, mut state) = state_with_temp_settings()?;

    // Act
    let result = state.act(id);

    // Assert
    assert_eq!(result, Ok(Effect::SetZoom(percent)));
    assert_eq!(state.settings.zoom_percent, percent);
    let (stored, warnings) = rivercrossing_store::load_settings(&state.settings_path);
    assert_eq!(stored.zoom_percent, percent);
    assert_eq!(warnings.len(), 0);
    Ok(())
}

#[test]
fn test_act_queues_the_undo_native_confirm() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    let mut ride = loaded_ride(RidePhase::Running)?
        .ok_or_else(|| "the running ride is not built".to_owned())?;
    let _ = ride.engine.record_crossing(PLATE, None);
    state.ride = Some(ride);

    // Act
    let result = state.act(MI_UNDO_CROSSING);

    // Assert
    assert_eq!(
        result,
        Ok(Effect::OpenNative {
            dialog: NativeDialog::Danger,
            then: CommandId::UndoCrossing,
        })
    );
    assert_eq!(
        state.pending_native,
        Some(PendingNative {
            dialog: NativeDialog::Danger,
            then: CommandId::UndoCrossing,
        })
    );
    Ok(())
}

#[test]
fn test_act_queues_a_plain_command() {
    // Arrange
    let mut state = empty_state();

    // Act
    let result = state.act(MI_BACKUP_NOW);

    // Assert
    assert_eq!(result, Ok(Effect::RunCommand(CommandId::BackupNow)));
    assert_eq!(state.pending_command, Some(CommandId::BackupNow));
}

#[test]
fn test_act_reports_a_window_not_built_in_this_release() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Draft)?;

    // Act
    let result = state.act(MI_RIDER_EDITOR);

    // Assert
    assert_eq!(
        result,
        Err(DispatchError::NotImplemented("rider_editor_dlg"))
    );
    assert!(state.dialogs.is_empty());
    Ok(())
}

#[rstest]
#[case(MI_NEW_RIDE, true)]
#[case(MI_SHORTCUTS, true)]
#[case(WX_ID_ABOUT, true)]
#[case(MI_BACKUP_NOW, true)]
#[case(MI_SHOW_TIMES, true)]
#[case(MI_ZOOM_100, true)]
#[case(WX_ID_EXIT, true)]
#[case(MI_RIDER_EDITOR, false)]
#[case(MI_STANDINGS, false)]
#[case(ActionId::from_name("not_an_action"), false)]
fn test_implemented_is_true_for_the_built_targets_and_false_for_the_rest(
    #[case] id: ActionId,
    #[case] expected: bool,
) {
    // Arrange
    let state = empty_state();

    // Act
    let implemented = state.implemented(id);

    // Assert
    assert_eq!(implemented, expected);
}

#[test]
fn test_a_failed_settings_save_keeps_the_toggle_and_posts_the_notice() -> Result<(), String> {
    // Arrange
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let blocker = directory.path().join("blocker");
    std::fs::write(&blocker, b"not a directory").map_err(|error| error.to_string())?;
    let mut state = empty_state();
    state.settings_path = blocker.join("settings.json");
    let before = state.settings.publish_show_times;
    let expected = rivercrossing_store::save_settings(&state.settings, &state.settings_path)
        .err()
        .map(|error| error.to_string())
        .ok_or_else(|| "the blocked path accepted a write".to_owned())?;

    // Act
    let result = state.act(MI_SHOW_TIMES);

    // Assert
    assert_eq!(result, Ok(Effect::ToggleCheck(CheckKind::PublishShowTimes)));
    assert_eq!(state.settings.publish_show_times, !before);
    assert_eq!(state.notice, Some(expected));
    Ok(())
}
