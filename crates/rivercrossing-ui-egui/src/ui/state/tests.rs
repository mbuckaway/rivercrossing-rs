// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the app state: the phase and gate projections, the status
//! notice, the settings write, the new-ride seed and the action execution.

use std::path::{Path, PathBuf};

use pretty_assertions::assert_eq;
use rivercrossing_ride::{
    EntryId, EntryMode, PlateModel, RideConfig, RideConfigParams, RideEngine, SystemClock,
};
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::{Entry, Rider};
use rivercrossing_store::Store;
use rstest::rstest;

use super::{
    AppState, ExportPresence, LoadedRide, NativeTask, PendingNative, RiderSelection, new_ride_seed,
};
use crate::ui::action::effect::{DispatchError, Effect};
use crate::ui::action::enable::{GateState, RidePhase};
use crate::ui::action::id::{
    ActionId, MI_ALL_CARDS, MI_BACKUP_NOW, MI_CHECK_RIDER_ISSUES, MI_FINISH_RIDE, MI_FULL_FIELD,
    MI_LAPS_BOARD, MI_NEW_RIDE, MI_PUBLISH_WORDPRESS, MI_RIDER_EDITOR, MI_SHORTCUTS,
    MI_SHOW_DNS_RIDERS, MI_SHOW_LAP_TIME, MI_SHOW_TIMES, MI_SHOW_TOTAL_TIMES, MI_STANDINGS,
    MI_TEAM_EDITOR, MI_TIME_BOARD, MI_UNDO_CROSSING, MI_ZOOM_90, MI_ZOOM_100, MI_ZOOM_110,
    MI_ZOOM_120, MI_ZOOM_130, MI_ZOOM_140, MI_ZOOM_150, WX_ID_ABOUT, WX_ID_EXIT,
};
use crate::ui::action::spec::{CheckKind, CommandId, NativeDialog};
use crate::ui::dialogs::stack::Focus;
use crate::ui::presenters::console::RIDE_STARTED;
use crate::ui::presenters::rider_issues;
use crate::ui::sound::Cue;
use crate::ui::worker::StoreCommand;

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
        min_lap_s: 1200,
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
    AppState::blank()
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
    loaded_ride_with(phase, &fixture_roster()?)
}

/// Build a loaded ride in *phase* over *roster*.
fn loaded_ride_with(phase: RidePhase, roster: &Roster) -> Result<Option<LoadedRide>, String> {
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
        .save_roster(ride_id, roster)
        .map_err(|error| error.to_string())?;
    let mut engine = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;
    advance_to(&mut engine, phase)?;
    let mut roster = store
        .roster_for(ride_id)
        .map_err(|error| error.to_string())?;
    roster.set_status(engine.state());
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
            then: NativeTask::Command(CommandId::UndoCrossing),
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

    // Act: the results dialog is not built in this release.
    let result = state.act(MI_STANDINGS);

    // Assert
    assert_eq!(result, Err(DispatchError::NotImplemented("results_dlg")));
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
#[case(MI_RIDER_EDITOR, true)]
#[case(MI_TEAM_EDITOR, true)]
#[case(MI_CHECK_RIDER_ISSUES, true)]
#[case(MI_STANDINGS, false)]
#[case(MI_PUBLISH_WORDPRESS, false)]
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

// ----------------------------------------------------------- setup dialog

/// Return the app state with the setup dialog open on `fields`.
fn setup_state(fields: &[(&str, &str)]) -> Result<AppState, String> {
    let mut state = empty_state();
    state
        .dialogs
        .open("ride_setup_dlg", None)
        .map_err(|_| "the setup dialog refuses to open".to_owned())?;
    for (field, value) in fields {
        state
            .dialogs
            .set_field(field, *value)
            .map_err(|_| format!("the setup dialog refuses `{field}`"))?;
    }
    Ok(state)
}

/// Return every setup field of a complete form.
fn complete_setup_fields() -> Vec<(&'static str, &'static str)> {
    vec![
        ("name_input", RIDE_NAME),
        ("date_picker", "2026-11-25"),
        ("start_time_picker", "08:00"),
        ("venue_input", "Sea to Sky Gondola"),
        ("lap_km_spin", "8.0"),
        ("organizer_input", "GORBA"),
        ("scorer_input", "K. Singh"),
        ("duration_input", "6:00"),
        ("min_lap_input", "20:00"),
    ]
}

#[test]
fn test_open_setup_dialog_seeds_its_defaults() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    let today = time::Date::from_calendar_date(2026, time::Month::November, 25)
        .map_err(|error| error.to_string())?;

    // Act
    state.open_setup_dialog(today);

    // Assert
    assert_eq!(
        state.dialogs.field("date_picker"),
        Some("2026-11-25".to_owned()).as_deref()
    );
    assert_eq!(state.dialogs.field("start_time_picker"), Some("08:00"));
    assert_eq!(state.dialogs.field("duration_input"), Some("6:00"));
    assert_eq!(state.dialogs.field("min_lap_input"), Some("20:00"));
    assert_eq!(state.dialogs.field("lap_km_spin"), Some("8.0"));
    assert_eq!(state.dialogs.field("team_size_spin"), Some("4"));
    assert_eq!(state.dialogs.field("decks_spin"), Some("8"));
    assert_eq!(state.dialogs.field("jokers_spin"), Some("1"));
    assert_eq!(state.dialogs.field("cap_choice"), Some("Disabled"));
    assert_eq!(state.dialogs.field("name_input"), None);
    Ok(())
}

#[test]
fn test_submit_setup_stores_the_pending_config() -> Result<(), String> {
    // Arrange
    let mut state = setup_state(&complete_setup_fields())?;

    // Act
    let result = state.submit_setup();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(
        state.pending_create.map(|config| config.name),
        Some(RIDE_NAME.to_owned())
    );
    assert_eq!(state.setup_errors, None);
    assert!(state.dialogs.is_empty());
    Ok(())
}

#[test]
fn test_submit_setup_reports_a_malformed_field() -> Result<(), String> {
    // Arrange
    let mut fields = complete_setup_fields();
    fields.push(("date_picker", "25/11/2026"));
    let mut state = setup_state(&fields)?;

    // Act
    let result = state.submit_setup();

    // Assert
    assert_eq!(
        result,
        Err(vec!["Date must be YYYY-MM-DD, got '25/11/2026'".to_owned()])
    );
    assert_eq!(state.pending_create, None);
    assert_eq!(
        state.setup_errors,
        Some(vec!["Date must be YYYY-MM-DD, got '25/11/2026'".to_owned()])
    );
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("ride_setup_dlg")
    );
    Ok(())
}

#[test]
fn test_submit_setup_reports_the_minimum_violations() -> Result<(), String> {
    // Arrange
    let mut fields = complete_setup_fields();
    fields.push(("venue_input", ""));
    let mut state = setup_state(&fields)?;

    // Act
    let result = state.submit_setup();

    // Assert
    assert_eq!(result, Err(vec!["venue is required".to_owned()]));
    assert_eq!(state.pending_create, None);
    Ok(())
}

#[test]
fn test_submit_setup_given_no_dialog_reports_it() {
    // Arrange
    let mut state = empty_state();

    // Act
    let result = state.submit_setup();

    // Assert
    assert_eq!(result, Err(vec!["No ride setup dialog is open".to_owned()]));
    assert_eq!(state.pending_create, None);
}

#[test]
fn test_submit_setup_given_another_dialog_reports_it() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state
        .dialogs
        .open("about_dlg", None)
        .map_err(|_| "the about dialog refuses to open".to_owned())?;

    // Act
    let result = state.submit_setup();

    // Assert
    assert_eq!(result, Err(vec!["No ride setup dialog is open".to_owned()]));
    Ok(())
}

// ------------------------------------------------------------ plate entry

#[test]
fn test_submit_plate_records_a_crossing_and_queues_the_cue() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Running)?;
    state.plate = PLATE.to_owned();

    // Act
    state.submit_plate();

    // Assert
    assert_eq!(
        state.notice,
        Some(format!(
            "Recorded plate {PLATE} — short lap flagged for review"
        ))
    );
    assert_eq!(state.pending_cues, vec![Cue::Flagged]);
    assert_eq!(state.plate, "");
    let crossings = state
        .ride
        .as_ref()
        .map(|ride| ride.engine.crossings().len());
    assert_eq!(crossings, Some(1));
    Ok(())
}

#[test]
fn test_submit_plate_given_a_blank_plate_is_silent_and_keeps_the_field() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Running)?;
    state.plate = "  ".to_owned();

    // Act
    state.submit_plate();

    // Assert
    assert_eq!(state.notice, None);
    assert_eq!(state.pending_cues, Vec::new());
    assert_eq!(state.plate, "  ");
    Ok(())
}

#[test]
fn test_submit_plate_given_an_unknown_plate_keeps_the_field_and_buzzes() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Running)?;
    state.plate = "999".to_owned();

    // Act
    state.submit_plate();

    // Assert
    assert_eq!(state.notice, Some("Unknown plate 999".to_owned()));
    assert_eq!(state.pending_cues, vec![Cue::Error]);
    assert_eq!(state.plate, "999");
    Ok(())
}

#[test]
fn test_submit_plate_without_a_ride_is_inert() {
    // Arrange
    let mut state = empty_state();
    state.plate = PLATE.to_owned();

    // Act
    state.submit_plate();

    // Assert
    assert_eq!(state.notice, None);
    assert_eq!(state.pending_cues, Vec::new());
    assert_eq!(state.plate, PLATE);
}

// ------------------------------------------------------------- lifecycle

#[test]
fn test_start_ride_posts_the_notice_and_clears_the_frozen_clock() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Draft)?;
    state.clock_frozen = Some(42.0);

    // Act
    state.start_ride();

    // Assert
    assert_eq!(state.notice, Some(RIDE_STARTED.to_owned()));
    assert_eq!(state.clock_frozen, None);
    assert_eq!(state.phase(), RidePhase::Running);
    Ok(())
}

#[test]
fn test_start_ride_given_a_blocked_draft_opens_the_blocked_dialog() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    let mut ride =
        loaded_ride(RidePhase::Draft)?.ok_or_else(|| "the draft ride is not built".to_owned())?;
    ride.engine
        .update_config(
            RideConfig::new(RideConfigParams {
                lap_km: 8.0,
                planned_duration_s: 21_600,
                min_lap_s: 1_200,
                ..RideConfigParams::default()
            })
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    state.ride = Some(ride);

    // Act
    state.start_ride();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("start_blocked_dlg")
    );
    assert!(
        state
            .notice
            .as_deref()
            .is_some_and(|notice| notice.contains("name is required")),
        "the notice names the blocked setup: {:?}",
        state.notice
    );
    Ok(())
}

#[test]
fn test_start_ride_without_a_ride_is_inert() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.start_ride();

    // Assert
    assert_eq!(state.notice, None);
}

#[test]
fn test_stop_ride_captures_the_frozen_clock() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Running)?;

    // Act
    state.stop_ride();

    // Assert
    assert_eq!(
        state.notice,
        Some("Ride stopped — continue to resume".to_owned())
    );
    assert!(
        state.clock_frozen.is_some_and(|frozen| frozen >= 0.0),
        "the stop captured the clock: {:?}",
        state.clock_frozen
    );
    assert_eq!(state.phase(), RidePhase::RunningStopped);
    Ok(())
}

#[test]
fn test_stop_ride_given_a_draft_posts_the_refusal() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Draft)?;

    // Act
    state.stop_ride();

    // Assert
    assert!(
        state
            .notice
            .as_deref()
            .is_some_and(|notice| notice.starts_with("Cannot stop:")),
        "the notice names the refusal: {:?}",
        state.notice
    );
    assert_eq!(state.clock_frozen, None);
    Ok(())
}

#[test]
fn test_stop_ride_without_a_ride_is_inert() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.stop_ride();

    // Assert
    assert_eq!(state.notice, None);
    assert_eq!(state.clock_frozen, None);
}

#[test]
fn test_undo_crossing_without_a_crossing_posts_the_refusal() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Running)?;

    // Act
    state.undo_crossing();

    // Assert
    assert_eq!(
        state.notice,
        Some("Undo unavailable: nothing to undo".to_owned())
    );
    Ok(())
}

#[test]
fn test_undo_crossing_given_no_ride_posts_the_refusal() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.undo_crossing();

    // Assert
    assert_eq!(
        state.notice,
        Some("Undo unavailable: nothing to undo".to_owned())
    );
}

#[test]
fn test_finish_and_reopen_post_the_notice() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Running)?;

    // Act
    state.finish_ride();

    // Assert
    assert_eq!(
        state.notice,
        Some("Ride is finished. Results are available".to_owned())
    );
    assert_eq!(state.phase(), RidePhase::Finished);

    // Act
    state.reopen_ride();

    // Assert
    assert_eq!(
        state.notice,
        Some("Ride reopened for corrections".to_owned())
    );
    assert_eq!(state.phase(), RidePhase::Reopened);
    Ok(())
}

#[test]
fn test_finish_ride_without_a_ride_is_inert() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.finish_ride();

    // Assert
    assert_eq!(state.notice, None);
}

#[test]
fn test_reopen_ride_without_a_ride_is_inert() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.reopen_ride();

    // Assert
    assert_eq!(state.notice, None);
}

#[test]
fn test_clear_ride_unloads_the_ride() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Running)?;
    state.audit_row_count = 3;
    state.exports = ExportPresence {
        html: true,
        poster_html: true,
        pdf: true,
    };

    // Act
    state.clear_ride();

    // Assert
    assert_eq!(state.notice, Some("Ride cleared".to_owned()));
    assert_eq!(state.phase(), RidePhase::None);
    assert_eq!(state.audit_row_count, 0);
    assert_eq!(state.exports, ExportPresence::default());
    assert_eq!(state.clock_frozen, None);
    Ok(())
}

#[test]
fn test_finish_ride_given_a_draft_posts_the_refusal() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Draft)?;

    // Act
    state.finish_ride();

    // Assert
    assert!(
        state
            .notice
            .as_deref()
            .is_some_and(|notice| notice.starts_with("Cannot finish:")),
        "the notice names the refusal: {:?}",
        state.notice
    );
    Ok(())
}

#[test]
fn test_reopen_ride_given_a_running_ride_posts_the_refusal() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Running)?;

    // Act
    state.reopen_ride();

    // Assert
    assert!(
        state
            .notice
            .as_deref()
            .is_some_and(|notice| notice.starts_with("Cannot reopen:")),
        "the notice names the refusal: {:?}",
        state.notice
    );
    Ok(())
}

// ------------------------------------------------------- native confirms

/// Return an app state whose running ride holds one crossing.
fn state_with_a_crossing() -> Result<AppState, String> {
    let mut state = empty_state();
    let mut ride = loaded_ride(RidePhase::Running)?
        .ok_or_else(|| "the running ride is not built".to_owned())?;
    let _ = ride.engine.record_crossing(PLATE, None);
    state.ride = Some(ride);
    Ok(state)
}

#[test]
fn test_confirm_native_forwards_the_command() -> Result<(), String> {
    // Arrange
    let mut state = state_with_a_crossing()?;
    state.act(MI_UNDO_CROSSING).map_err(|e| format!("{e:?}"))?;

    // Act
    let confirmed = state.confirm_native();

    // Assert
    assert!(confirmed);
    assert_eq!(state.pending_native, None);
    assert_eq!(state.pending_command, Some(CommandId::UndoCrossing));
    assert_eq!(
        state
            .ride
            .as_ref()
            .map(|ride| ride.engine.crossings().len()),
        Some(1),
        "confirming queues the command; the app runs it on the next pass"
    );
    Ok(())
}

#[test]
fn test_cancel_native_runs_nothing() -> Result<(), String> {
    // Arrange
    let mut state = state_with_a_crossing()?;
    state.act(MI_UNDO_CROSSING).map_err(|e| format!("{e:?}"))?;

    // Act
    let cancelled = state.cancel_native();

    // Assert
    assert!(cancelled);
    assert_eq!(state.pending_native, None);
    assert_eq!(state.pending_command, None);
    assert_eq!(
        state
            .ride
            .as_ref()
            .map(|ride| ride.engine.crossings().len()),
        Some(1)
    );
    Ok(())
}

#[test]
fn test_confirm_and_cancel_native_without_a_pending_dialog_report_false() {
    // Arrange
    let mut state = empty_state();

    // Act
    let confirmed = state.confirm_native();
    let cancelled = state.cancel_native();

    // Assert
    assert!(!confirmed);
    assert!(!cancelled);
    assert_eq!(state.pending_command, None);
}

// ------------------------------------------------------ editor fixtures

/// Return a rider with the given name and own plate.
fn rider(first: &str, last: &str, plate: Option<&str>) -> Rider {
    Rider {
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        plate: plate.map(str::to_owned),
        ..Rider::default()
    }
}

/// Return a restored solo entry.
#[expect(
    clippy::too_many_arguments,
    reason = "one fixture call spells the entry, its plate and its rider"
)]
fn solo_entry(key: &str, plate: &str, first: &str, last: &str) -> Entry {
    Entry::restored(
        key,
        plate,
        format!("{first} {last}").trim().to_owned(),
        EntryType::Solo,
        vec![rider(first, last, Some(plate))],
    )
}

/// Return a restored team entry.
#[expect(
    clippy::too_many_arguments,
    reason = "one fixture call spells the team, its plate, its name and its members"
)]
fn team_entry(key: &str, plate: &str, name: &str, members: Vec<Rider>) -> Entry {
    Entry::restored(key, plate, name, EntryType::Team, members)
}

/// Return a roster with a solo pair and two teams.
fn editor_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster.load_entries(vec![
        solo_entry("k-ada", "101", "Ada", "Lovelace"),
        solo_entry("k-grace", "102", "Grace", "Hopper"),
        team_entry(
            "k-alpha",
            "A1",
            "Alpha",
            vec![
                rider("Alan", "Turing", Some("201")),
                rider("Barbara", "Liskov", Some("202")),
            ],
        ),
        team_entry(
            "k-lone",
            "L1",
            "Lone Wolf",
            vec![rider("Cy", "Three", Some("103"))],
        ),
    ]);
    Ok(roster)
}

/// Return a roster that starts cleanly: no team-of-one, every rider plated and named.
fn running_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster.load_entries(vec![
        solo_entry("k-ada", "101", "Ada", "Lovelace"),
        solo_entry("k-grace", "102", "Grace", "Hopper"),
        team_entry(
            "k-alpha",
            "A1",
            "Alpha",
            vec![
                rider("Alan", "Turing", Some("201")),
                rider("Barbara", "Liskov", Some("202")),
            ],
        ),
    ]);
    Ok(roster)
}

/// Return a roster that carries one of each supported defect.
fn issues_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster
        .create_team_entry_of_one(TeamFields {
            display_name: "Lone Wolf",
            riders: vec![rider("Cy", "Three", Some("101"))],
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    roster.load_entries(vec![
        Entry::restored(
            "k-missing-name",
            "7",
            "",
            EntryType::Solo,
            vec![rider("", "", Some("7"))],
        ),
        Entry::restored(
            "k-missing-number",
            "9",
            "Grace Hopper",
            EntryType::Solo,
            vec![rider("Grace", "Hopper", None)],
        ),
        Entry::restored(
            "k-dup-number-1",
            "31",
            "Bo One",
            EntryType::Solo,
            vec![rider("Bo", "One", Some("21"))],
        ),
        Entry::restored(
            "k-dup-number-2",
            "32",
            "Bo Two",
            EntryType::Solo,
            vec![rider("Bo", "Two", Some("21"))],
        ),
    ]);
    Ok(roster)
}

/// Return an app state whose ride carries *roster*, in *phase*.
fn state_with_roster(phase: RidePhase, roster: &Roster) -> Result<AppState, String> {
    let mut state = empty_state();
    state.ride = loaded_ride_with(phase, roster)?;
    Ok(state)
}

/// Select one rider record.
fn select_rider(state: &mut AppState, entry: &str, rider_index: usize) {
    state.rider_editor.selected = Some(RiderSelection {
        entry: EntryId(entry.to_owned()),
        rider_index,
    });
}

/// Return the queued roster-audit actions.
fn queued_roster_events(state: &AppState) -> Vec<String> {
    state
        .pending_store
        .iter()
        .filter_map(|command| match command {
            StoreCommand::AppendRosterEvent { event, .. } => Some(event.action.clone()),
            _ => None,
        })
        .collect()
}

/// Return how many roster saves the state queued.
fn queued_saves(state: &AppState) -> usize {
    state
        .pending_store
        .iter()
        .filter(|command| matches!(command, StoreCommand::SaveRoster { .. }))
        .count()
}

/// Return how many ride reloads the state queued.
fn queued_loads(state: &AppState) -> usize {
    state
        .pending_store
        .iter()
        .filter(|command| matches!(command, StoreCommand::LoadRide { .. }))
        .count()
}

/// Write a CSV file with the canonical header and return its path.
fn write_csv(directory: &tempfile::TempDir, name: &str, rows: &str) -> Result<PathBuf, String> {
    let path = directory.path().join(name);
    let body = format!("FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\n{rows}");
    std::fs::write(&path, body).map_err(|error| error.to_string())?;
    Ok(path)
}

// -------------------------------------------------------- rider editor

#[test]
fn test_open_rider_editor_lists_every_rider() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;

    // Act
    state.open_rider_editor();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("rider_editor_dlg")
    );
    assert_eq!(state.rider_editor.rows.len(), 5);
    assert_eq!(state.rider_editor.rows[0].row.name, "Ada Lovelace");
    assert_eq!(state.rider_editor.rows[0].row.team, "solo");
    assert_eq!(state.rider_editor.rows[2].row.team, "Alpha");
    assert_eq!(state.rider_editor.selected, None);
    Ok(())
}

#[test]
fn test_open_rider_at_selects_the_plate() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;

    // Act
    state.open_rider_at("102");

    // Assert
    assert_eq!(
        state.rider_editor.selected,
        Some(RiderSelection {
            entry: EntryId("k-grace".to_owned()),
            rider_index: 0,
        })
    );
    Ok(())
}

#[test]
fn test_the_rider_editor_visible_rows_follow_the_search() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();

    // Act
    state.rider_editor.search = "alpha".to_owned();
    let visible = state.rider_editor.visible();

    // Assert
    assert_eq!(visible.len(), 2);
    assert_eq!(visible, vec![2, 3]);
    Ok(())
}

#[test]
fn test_open_add_rider_prefills_the_next_free_plate() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    let expected = state
        .ride
        .as_ref()
        .map(|ride| ride.roster.next_free_plate())
        .ok_or_else(|| "the ride is not loaded".to_owned())?;

    // Act
    state.open_add_rider();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("add_rider_dlg")
    );
    assert_eq!(state.dialogs.field("plate_input"), Some(expected.as_str()));
    assert_eq!(state.rider_editor.editing, None);
    Ok(())
}

#[test]
fn test_submit_add_rider_creates_and_selects_the_record() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();
    state.open_add_rider();
    state
        .dialogs
        .set_field("first_name_input", "Katherine")
        .map_err(|_| "the plate field".to_owned())?;
    state
        .dialogs
        .set_field("last_name_input", "Johnson")
        .map_err(|_| "the name field".to_owned())?;

    // Act
    let result = state.submit_add_rider();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(state.rider_editor.rows.len(), 6);
    let selected = state
        .rider_editor
        .selected
        .clone()
        .ok_or_else(|| "the new record is selected".to_owned())?;
    let ride = state.ride.as_ref().ok_or_else(|| "the ride".to_owned())?;
    let entry = ride
        .roster
        .entry_by_key(&selected.entry.0)
        .ok_or_else(|| "the new entry".to_owned())?;
    assert_eq!(entry.riders[selected.rider_index].first_name, "Katherine");
    assert_eq!(state.pending_store.len(), 3);
    assert_eq!(queued_saves(&state), 1);
    assert_eq!(queued_loads(&state), 1);
    assert_eq!(state.notice, Some("Rider added".to_owned()));
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("rider_editor_dlg")
    );
    Ok(())
}

#[test]
fn test_submit_add_rider_given_blank_names_refuses() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();
    state.open_add_rider();

    // Act
    let result = state.submit_add_rider();

    // Assert
    assert_eq!(
        result,
        Err("First name and last name are required".to_owned())
    );
    assert_eq!(
        state.rider_editor.errors.as_deref(),
        Some("First name and last name are required")
    );
    assert_eq!(state.rider_editor.rows.len(), 5);
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("add_rider_dlg")
    );
    assert!(state.pending_store.is_empty());
    Ok(())
}

#[test]
fn test_open_edit_rider_preloads_the_selected_record() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();
    select_rider(&mut state, "k-grace", 0);

    // Act
    state.open_edit_rider();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("add_rider_dlg")
    );
    assert_eq!(state.dialogs.field("plate_input"), Some("102"));
    assert_eq!(state.dialogs.field("first_name_input"), Some("Grace"));
    assert_eq!(state.dialogs.field("last_name_input"), Some("Hopper"));
    assert_eq!(state.dialogs.field("team_choice"), Some("solo"));
    assert_eq!(
        state.rider_editor.editing,
        Some(RiderSelection {
            entry: EntryId("k-grace".to_owned()),
            rider_index: 0,
        })
    );
    Ok(())
}

#[test]
fn test_submit_edit_rider_updates_the_roster() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();
    select_rider(&mut state, "k-grace", 0);
    state.open_edit_rider();
    state
        .dialogs
        .set_field("first_name_input", "Augusta")
        .map_err(|_| "the name field".to_owned())?;

    // Act
    let result = state.submit_edit_rider();

    // Assert
    assert_eq!(result, Ok(()));
    let ride = state.ride.as_ref().ok_or_else(|| "the ride".to_owned())?;
    let entry = ride
        .roster
        .entry_by_key("k-grace")
        .ok_or_else(|| "the entry".to_owned())?;
    assert_eq!(entry.riders[0].first_name, "Augusta");
    assert_eq!(state.notice, Some("Rider updated".to_owned()));
    assert_eq!(state.rider_editor.editing, None);
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("rider_editor_dlg")
    );
    Ok(())
}

#[test]
fn test_submit_edit_rider_given_a_running_ride_team_change_refuses_the_live_move()
-> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Running, &running_roster()?)?;
    state.open_rider_editor();
    select_rider(&mut state, "k-ada", 0);
    state.open_edit_rider();
    state
        .dialogs
        .set_field("team_choice", "Alpha")
        .map_err(|_| "the team field".to_owned())?;

    // Act
    let result = state.submit_edit_rider();

    // Assert
    assert_eq!(result, Err("Stop the ride first".to_owned()));
    assert_eq!(
        state.rider_editor.errors.as_deref(),
        Some("Stop the ride first")
    );
    let ride = state.ride.as_ref().ok_or_else(|| "the ride".to_owned())?;
    assert!(
        ride.roster.entry_by_key("k-ada").is_some(),
        "the rider stays on the solo entry"
    );
    Ok(())
}

#[test]
fn test_submit_edit_rider_given_no_selection_refuses() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();
    state.open_add_rider();

    // Act
    let result = state.submit_edit_rider();

    // Assert
    assert_eq!(result, Err("Select a rider first".to_owned()));
    Ok(())
}

#[test]
fn test_delete_selected_rider_removes_the_record() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();
    select_rider(&mut state, "k-grace", 0);

    // Act
    let result = state.delete_selected_rider();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(state.rider_editor.rows.len(), 4);
    assert_eq!(state.rider_editor.selected, None);
    let ride = state.ride.as_ref().ok_or_else(|| "the ride".to_owned())?;
    assert!(ride.roster.entry_by_key("k-grace").is_none());
    assert_eq!(state.notice, Some("Rider deleted".to_owned()));
    assert_eq!(queued_saves(&state), 1);
    Ok(())
}

#[test]
fn test_delete_selected_rider_given_no_selection_refuses() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();

    // Act
    let result = state.delete_selected_rider();

    // Assert
    assert_eq!(result, Err("Select a rider first".to_owned()));
    Ok(())
}

#[test]
fn test_delete_selected_rider_on_a_running_ride_surfaces_the_lock() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Running, &running_roster()?)?;
    state.open_rider_editor();
    select_rider(&mut state, "k-ada", 0);

    // Assert the fixture: the UI roster mirrors the running engine.
    let status = state
        .ride
        .as_ref()
        .map(|ride| ride.roster.status())
        .ok_or_else(|| "the ride".to_owned())?;
    assert_eq!(status, rivercrossing_roster::enums::RideStatus::Running);

    // Act
    let result = state.delete_selected_rider();

    // Assert
    assert!(result.is_err(), "the running ride locks the delete");
    assert_eq!(state.rider_editor.rows.len(), 4);
    assert!(state.rider_editor.errors.is_some());
    Ok(())
}

#[test]
fn test_raise_delete_rider_queues_the_confirmation() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();
    select_rider(&mut state, "k-grace", 0);

    // Act
    let raised = state.raise_delete_rider();

    // Assert
    assert!(raised);
    assert_eq!(
        state.pending_native,
        Some(PendingNative {
            dialog: NativeDialog::Danger,
            then: NativeTask::DeleteRider {
                entry: EntryId("k-grace".to_owned()),
                rider_index: 0,
            },
        })
    );
    Ok(())
}

#[test]
fn test_confirm_native_deletes_the_rider() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();
    select_rider(&mut state, "k-grace", 0);
    state.raise_delete_rider();

    // Act
    let confirmed = state.confirm_native();

    // Assert
    assert!(confirmed);
    assert_eq!(state.pending_native, None);
    assert_eq!(state.rider_editor.rows.len(), 4);
    assert_eq!(state.notice, Some("Rider deleted".to_owned()));
    Ok(())
}

#[test]
fn test_cancel_native_keeps_the_rider() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();
    select_rider(&mut state, "k-grace", 0);
    state.raise_delete_rider();

    // Act
    let cancelled = state.cancel_native();

    // Assert
    assert!(cancelled);
    assert_eq!(state.pending_native, None);
    assert_eq!(state.rider_editor.rows.len(), 5);
    assert_eq!(state.pending_store.len(), 0);
    Ok(())
}

// ----------------------------------------------------------------- csv

#[test]
fn test_preview_csv_opens_the_dialog_and_enables_import() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = write_csv(
        &directory,
        "entries.csv",
        "Katherine,Johnson,solo,,111,,\nDorothy,Vaughan,solo,,112,,\n",
    )?;

    // Act
    let result = state.preview_csv(&path);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("csv_preview_dlg")
    );
    let model = state
        .csv_preview
        .model
        .as_ref()
        .ok_or_else(|| "the preview model".to_owned())?;
    assert!(model.can_import);
    assert_eq!(model.conflicts, Vec::<String>::new());
    assert_eq!(state.csv_preview.path.as_deref(), Some(path.as_path()));
    Ok(())
}

#[test]
fn test_preview_csv_given_a_conflict_disables_import() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = write_csv(
        &directory,
        "entries.csv",
        "Katherine,Johnson,solo,,111,,\nDorothy,Vaughan,solo,,111,,\n",
    )?;

    // Act
    let result = state.preview_csv(&path);

    // Assert
    assert_eq!(result, Ok(()));
    let model = state
        .csv_preview
        .model
        .as_ref()
        .ok_or_else(|| "the preview model".to_owned())?;
    assert!(!model.can_import);
    assert_eq!(model.conflicts, vec!["duplicate plate 111".to_owned()]);
    Ok(())
}

#[test]
fn test_preview_csv_given_a_missing_file_refuses() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    let path = PathBuf::from("/no/such/entries.csv");

    // Act
    let result = state.preview_csv(&path);

    // Assert
    assert!(result.is_err());
    assert!(state.csv_preview.errors.is_some());
    assert_eq!(state.dialogs.top(), None);
    Ok(())
}

#[test]
fn test_submit_csv_import_commits_the_preview() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = write_csv(
        &directory,
        "entries.csv",
        "Katherine,Johnson,solo,,111,,\nDorothy,Vaughan,solo,,112,,\n",
    )?;
    state.preview_csv(&path)?;

    // Act
    let result = state.submit_csv_import();

    // Assert
    assert_eq!(result, Ok(()));
    let ride = state.ride.as_ref().ok_or_else(|| "the ride".to_owned())?;
    assert_eq!(ride.roster.entries().len(), 6);
    assert_eq!(state.notice, Some("Imported 2 entries".to_owned()));
    assert_eq!(state.dialogs.top(), None);
    assert_eq!(state.csv_preview.preview, None);
    assert_eq!(queued_saves(&state), 1);
    assert_eq!(queued_loads(&state), 1);
    Ok(())
}

#[test]
fn test_submit_csv_import_given_no_preview_refuses() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;

    // Act
    let result = state.submit_csv_import();

    // Assert
    assert_eq!(result, Err("No CSV file is previewed".to_owned()));
    Ok(())
}

#[test]
fn test_set_csv_option_writes_the_field_and_repreviews() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = write_csv(&directory, "entries.csv", "Ada,Lovelace,solo,,111,,\n")?;
    state.preview_csv(&path)?;

    // Act
    state.set_csv_option("map_unknown_sex_chk", true);

    // Assert
    assert_eq!(state.dialogs.field("map_unknown_sex_chk"), Some("true"));
    assert!(state.csv_preview.model.is_some());
    Ok(())
}

#[test]
fn test_export_csv_writes_the_roster() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("roster.csv");

    // Act
    let result = state.export_csv(&path);

    // Assert
    assert_eq!(result, Ok(()));
    let body = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    assert!(
        body.starts_with("FIRSTNAME,LASTNAME,TYPE"),
        "the export holds the canonical header: {body:?}"
    );
    assert!(
        state
            .notice
            .is_some_and(|notice| notice.starts_with("Exported "))
    );
    Ok(())
}

#[test]
fn test_export_csv_given_no_ride_refuses() {
    // Arrange
    let mut state = empty_state();

    // Act
    let result = state.export_csv(Path::new("/tmp/roster.csv"));

    // Assert
    assert_eq!(result, Err("Open a ride first".to_owned()));
}

// --------------------------------------------------------------- teams

/// Select the team named *name*.
fn select_team(state: &mut AppState, name: &str) {
    state.team_editor.selected = state
        .team_editor
        .rows
        .iter()
        .find(|row| row.row.name == name)
        .map(|row| row.entry.clone());
}

#[test]
fn test_open_team_editor_lists_the_teams() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;

    // Act
    state.open_team_editor();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("team_editor_dlg")
    );
    let names: Vec<&str> = state
        .team_editor
        .rows
        .iter()
        .map(|row| row.row.name.as_str())
        .collect();
    assert_eq!(names, vec!["Alpha", "Lone Wolf"]);
    assert_eq!(state.team_editor.selected, None);
    Ok(())
}

#[test]
fn test_set_team_single_member_only_filters_the_rows() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_team_editor();

    // Act
    state.set_team_single_member_only(true);

    // Assert
    assert_eq!(state.team_editor.rows.len(), 1);
    assert_eq!(state.team_editor.rows[0].row.name, "Lone Wolf");
    assert_eq!(state.dialogs.field("single_member_only_chk"), Some("true"));
    Ok(())
}

#[test]
fn test_submit_add_team_creates_and_selects_the_team() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_team_editor();
    state.open_add_team();
    state
        .dialogs
        .set_field("name_input", "Beta")
        .map_err(|_| "the name field".to_owned())?;

    // Act
    let result = state.submit_add_team();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(state.team_editor.rows.len(), 3);
    assert!(state.team_editor.selected.is_some());
    assert_eq!(state.notice, Some("Team added".to_owned()));
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("team_editor_dlg")
    );
    assert_eq!(queued_saves(&state), 1);
    Ok(())
}

#[test]
fn test_submit_add_team_given_a_blank_name_refuses() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_team_editor();
    state.open_add_team();

    // Act
    let result = state.submit_add_team();

    // Assert
    assert_eq!(result, Err("enter a team name".to_owned()));
    assert_eq!(
        state.team_editor.errors.as_deref(),
        Some("enter a team name")
    );
    assert_eq!(state.team_editor.rows.len(), 2);
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("add_team_dlg")
    );
    Ok(())
}

#[test]
fn test_open_edit_team_preloads_the_selected_team() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_team_editor();
    select_team(&mut state, "Alpha");

    // Act
    state.open_edit_team();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("add_team_dlg")
    );
    assert_eq!(state.dialogs.field("name_input"), Some("Alpha"));
    assert_eq!(state.dialogs.field("relay_plate_input"), Some("A1"));
    assert_eq!(state.team_editor.editing.is_some(), true);
    Ok(())
}

#[test]
fn test_submit_edit_team_renames_the_team() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_team_editor();
    select_team(&mut state, "Alpha");
    state.open_edit_team();
    state
        .dialogs
        .set_field("name_input", "Alpha Prime")
        .map_err(|_| "the name field".to_owned())?;

    // Act
    let result = state.submit_edit_team();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(state.notice, Some("Team updated".to_owned()));
    let ride = state.ride.as_ref().ok_or_else(|| "the ride".to_owned())?;
    let names: Vec<&str> = ride
        .roster
        .entries()
        .iter()
        .map(|entry| entry.display_name.as_str())
        .collect();
    assert!(
        names.contains(&"Alpha Prime"),
        "the team was renamed: {names:?}"
    );
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("team_editor_dlg")
    );
    Ok(())
}

#[test]
fn test_pick_team_card_draws_a_logo() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_team_editor();
    select_team(&mut state, "Alpha");

    // Act
    let result = state.pick_team_card();

    // Assert
    assert_eq!(result, Ok(()));
    let ride = state.ride.as_ref().ok_or_else(|| "the ride".to_owned())?;
    let entry = ride
        .roster
        .entry_by_key("k-alpha")
        .ok_or_else(|| "the team".to_owned())?;
    assert!(entry.logo_card.is_some(), "the team picked a card");
    assert!(
        state
            .notice
            .is_some_and(|notice| notice.starts_with("Logo card "))
    );
    Ok(())
}

#[test]
fn test_pick_team_card_given_no_selection_refuses() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_team_editor();

    // Act
    let result = state.pick_team_card();

    // Assert
    assert_eq!(result, Err("Select a team first".to_owned()));
    Ok(())
}

#[test]
fn test_remove_selected_team_removes_it() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_team_editor();
    select_team(&mut state, "Alpha");

    // Act
    let result = state.remove_selected_team();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(state.team_editor.rows.len(), 1);
    assert_eq!(state.team_editor.selected, None);
    assert_eq!(state.notice, Some("Team removed".to_owned()));
    assert_eq!(queued_saves(&state), 1);
    Ok(())
}

#[test]
fn test_remove_selected_team_on_a_running_ride_surfaces_the_lock() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Running, &running_roster()?)?;
    state.open_team_editor();
    select_team(&mut state, "Alpha");

    // Act
    let result = state.remove_selected_team();

    // Assert
    assert!(result.is_err(), "the running ride locks the removal");
    assert_eq!(state.team_editor.rows.len(), 1);
    assert!(state.team_editor.errors.is_some());
    Ok(())
}

#[test]
fn test_raise_and_confirm_remove_team_removes_after_the_confirmation() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_team_editor();
    select_team(&mut state, "Alpha");

    // Act
    let raised = state.raise_remove_team();
    let cancelled = state.cancel_native();
    let raised_again = state.raise_remove_team();
    let confirmed = state.confirm_native();

    // Assert
    assert!(raised);
    assert!(cancelled);
    assert!(raised_again);
    assert!(confirmed);
    assert_eq!(state.pending_native, None);
    assert_eq!(state.team_editor.rows.len(), 1);
    assert_eq!(state.notice, Some("Team removed".to_owned()));
    Ok(())
}

// -------------------------------------------------------------- issues

/// Return the first cached issue index of *kind*.
fn issue_index(state: &AppState, kind: rider_issues::IssueKind) -> Option<usize> {
    state
        .rider_issues
        .rows
        .iter()
        .position(|row| row.kind == kind)
}

/// Return the last cached issue index of *kind*.
fn last_issue_index(state: &AppState, kind: rider_issues::IssueKind) -> Option<usize> {
    state
        .rider_issues
        .rows
        .iter()
        .rposition(|row| row.kind == kind)
}

#[test]
fn test_open_rider_issues_lists_the_supported_defects() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &issues_roster()?)?;

    // Act
    state.open_rider_issues();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("rider_issues_dlg")
    );
    let count = |kind| {
        state
            .rider_issues
            .rows
            .iter()
            .filter(|row| row.kind == kind)
            .count()
    };
    assert_eq!(count(rider_issues::IssueKind::TeamOfOne), 1);
    assert_eq!(count(rider_issues::IssueKind::MissingName), 1);
    assert_eq!(count(rider_issues::IssueKind::MissingNumber), 1);
    assert_eq!(count(rider_issues::IssueKind::DuplicateNumber), 1);
    assert_eq!(state.rider_issues.selected, None);
    Ok(())
}

#[test]
fn test_convert_issue_to_solo_converts_the_team_of_one() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &issues_roster()?)?;
    state.open_rider_issues();
    state.rider_issues.selected = issue_index(&state, rider_issues::IssueKind::TeamOfOne);

    // Act
    let result = state.convert_issue_to_solo();

    // Assert
    assert_eq!(result, Ok(()));
    let ride = state.ride.as_ref().ok_or_else(|| "the ride".to_owned())?;
    let teams = ride
        .roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Team)
        .count();
    assert_eq!(teams, 0, "the team-of-one dissolved into a solo entry");
    assert_eq!(
        state.notice,
        Some("Converted the team-of-one to solo".to_owned())
    );
    assert_eq!(
        state
            .rider_issues
            .rows
            .iter()
            .filter(|row| row.kind == rider_issues::IssueKind::TeamOfOne)
            .count(),
        0
    );
    Ok(())
}

#[test]
fn test_convert_issue_to_solo_given_no_selection_refuses() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &issues_roster()?)?;
    state.open_rider_issues();

    // Act
    let result = state.convert_issue_to_solo();

    // Assert
    assert_eq!(result, Err("Select an issue first".to_owned()));
    Ok(())
}

#[test]
fn test_assign_issue_plate_gives_the_next_free_plate() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &issues_roster()?)?;
    state.open_rider_issues();
    state.rider_issues.selected = issue_index(&state, rider_issues::IssueKind::MissingNumber);
    let expected = state
        .ride
        .as_ref()
        .map(|ride| ride.roster.next_free_plate())
        .ok_or_else(|| "the ride".to_owned())?;

    // Act
    let result = state.assign_issue_plate();

    // Assert
    assert_eq!(result, Ok(()));
    let ride = state.ride.as_ref().ok_or_else(|| "the ride".to_owned())?;
    let entry = ride
        .roster
        .entry_by_key("k-missing-number")
        .ok_or_else(|| "the entry".to_owned())?;
    assert_eq!(entry.riders[0].plate.as_deref(), Some(expected.as_str()));
    assert_eq!(state.notice, Some(format!("Assigned plate {expected}")));
    Ok(())
}

#[test]
fn test_renumber_issue_resolves_the_duplicate() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &issues_roster()?)?;
    state.open_rider_issues();
    state.rider_issues.selected =
        last_issue_index(&state, rider_issues::IssueKind::DuplicateNumber);

    // Act
    let result = state.renumber_issue();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(
        state
            .rider_issues
            .rows
            .iter()
            .filter(|row| row.kind == rider_issues::IssueKind::DuplicateNumber)
            .count(),
        0
    );
    assert!(
        state
            .notice
            .is_some_and(|notice| notice.starts_with("Renumbered to "))
    );
    Ok(())
}

#[test]
fn test_open_issue_editor_opens_the_rider_editor_at_the_plate() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &issues_roster()?)?;
    state.open_rider_issues();
    state.rider_issues.selected = issue_index(&state, rider_issues::IssueKind::MissingName);

    // Act
    state.open_issue_editor();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("rider_editor_dlg")
    );
    assert_eq!(
        state
            .rider_editor
            .selected
            .as_ref()
            .map(|selection| selection.entry.0.clone()),
        Some("k-missing-name".to_owned())
    );
    Ok(())
}

#[test]
fn test_open_issue_editor_for_a_team_of_one_opens_the_teams_editor() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &issues_roster()?)?;
    state.open_rider_issues();
    state.rider_issues.selected = issue_index(&state, rider_issues::IssueKind::TeamOfOne);

    // Act
    state.open_issue_editor();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("team_editor_dlg")
    );
    assert_eq!(
        state
            .team_editor
            .selected
            .as_ref()
            .and_then(|entry| state
                .team_editor
                .rows
                .iter()
                .find(|row| row.entry == *entry))
            .map(|row| row.row.name.clone()),
        Some("Lone Wolf".to_owned())
    );
    Ok(())
}

// ------------------------------------------------------------ audit drain

#[test]
fn test_a_roster_mutation_drains_its_audit_rows_and_saves() -> Result<(), String> {
    // Arrange
    let mut state = state_with_roster(RidePhase::Draft, &editor_roster()?)?;
    state.open_rider_editor();
    state.open_add_rider();
    state
        .dialogs
        .set_field("first_name_input", "Katherine")
        .map_err(|_| "the name field".to_owned())?;
    state
        .dialogs
        .set_field("last_name_input", "Johnson")
        .map_err(|_| "the name field".to_owned())?;

    // Act
    state.submit_add_rider()?;

    // Assert
    let ride = state.ride.as_ref().ok_or_else(|| "the ride".to_owned())?;
    assert!(
        ride.roster.audit_log().is_empty(),
        "the mutation's audit rows left the roster"
    );
    assert!(
        !queued_roster_events(&state).is_empty(),
        "the audit rows are queued for the store"
    );
    assert_eq!(queued_saves(&state), 1);
    assert_eq!(queued_loads(&state), 1);
    Ok(())
}
