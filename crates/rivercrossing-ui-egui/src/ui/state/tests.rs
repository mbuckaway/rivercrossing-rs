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

use super::exports::ExportKind;
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
use crate::ui::presenters::simulator;
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
        last_html: None,
        last_poster_html: None,
        last_pdf: None,
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
fn test_act_opens_the_results_window_in_any_state() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Draft)?;

    // Act
    let result = state.act(MI_STANDINGS);

    // Assert
    assert_eq!(result, Ok(Effect::OpenWindow("results_dlg")));
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("results_dlg")
    );
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
#[case(MI_STANDINGS, true)]
#[case(MI_PUBLISH_WORDPRESS, true)]
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
        last_html: None,
        last_poster_html: None,
        last_pdf: None,
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

// ------------------------------------------------- lifecycle & system

use rivercrossing_ride::RideStatus;
use rivercrossing_store::session::SessionState;
use rivercrossing_store::{AuditRow, PreviousSession, RideRow};

/// Return a stored ride row.
fn ride_row(id: i64, name: &str, status: RideStatus) -> RideRow {
    RideRow {
        id,
        name: name.to_owned(),
        event_date: time::Date::from_calendar_date(2026, time::Month::September, 20)
            .expect("the fixture date"),
        status,
        entries: 1,
    }
}

/// Return a running state whose ride holds one crossing on k-ada.
fn running_with_crossing() -> Result<AppState, String> {
    let mut state = state_with_roster(RidePhase::Draft, &running_roster()?)?;
    let start = epoch_start();
    if let Some(ride) = state.ride.as_mut() {
        ride.engine
            .start(Some(start))
            .map_err(|error| error.to_string())?;
        ride.roster.set_status(ride.engine.state());
        let _ = ride
            .engine
            .record_crossing("101", Some(start + time::Duration::hours(1)));
    }
    Ok(state)
}

/// Return the epoch instant that the correction fixtures pin, so none reads the wall clock.
fn epoch_start() -> rivercrossing_ride::Timestamp {
    let date = time::Date::from_calendar_date(1970, time::Month::January, 1)
        .expect("the epoch date is valid");
    time::OffsetDateTime::new_utc(date, time::Time::MIDNIGHT)
}

// ------------------------------------------------------------ native

#[test]
fn test_run_native_task_queues_a_duplicate() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.run_native_task(NativeTask::DuplicateRide { id: 4 });

    // Assert
    assert!(matches!(
        state.pending_store.first(),
        Some(StoreCommand::DuplicateRide { id: 4 })
    ));
}

#[test]
fn test_run_native_task_deletes_a_crossing() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;

    // Act
    state.run_native_task(NativeTask::DeleteCrossing {
        entry: EntryId("k-ada".to_owned()),
        seq: 1,
    });

    // Assert
    let crossings = state
        .ride
        .as_ref()
        .map(|ride| ride.engine.crossings().len());
    assert_eq!(crossings, Some(0));
    assert!(
        state
            .pending_store
            .iter()
            .any(|command| matches!(command, StoreCommand::Append { .. }))
    );
    Ok(())
}

#[test]
fn test_native_copy_names_each_task() {
    // Arrange
    let mut state = empty_state();
    state.set_library_rows(vec![ride_row(4, "Club poker night", RideStatus::Finished)]);

    // Act
    let duplicate = state.native_message(&PendingNative {
        dialog: NativeDialog::Prompt,
        then: NativeTask::DuplicateRide { id: 4 },
    });
    let quit = state.native_message(&PendingNative {
        dialog: NativeDialog::Confirm,
        then: NativeTask::Command(CommandId::AppQuit),
    });

    // Assert
    assert!(duplicate.contains("Club poker night"), "{duplicate}");
    assert_eq!(quit, "Quit RiverCrossing?");
    assert_eq!(
        AppState::native_confirm_label(&PendingNative {
            dialog: NativeDialog::Prompt,
            then: NativeTask::DuplicateRide { id: 4 },
        }),
        "Duplicate"
    );
    assert_eq!(AppState::native_title(NativeDialog::Danger), "Warning");
}

// ------------------------------------------------------------ library

#[test]
fn test_open_library_opens_and_asks_for_the_rows() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.open_library();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("ride_library_dlg")
    );
    assert!(matches!(
        state.pending_store.first(),
        Some(StoreCommand::Rides)
    ));
}

#[test]
fn test_set_library_rows_renders_and_clamps_the_selection() {
    // Arrange
    let mut state = empty_state();
    state.library.selected = Some(5);

    // Act
    state.set_library_rows(vec![
        ride_row(1, "GORBA EPIC 2026", RideStatus::Running),
        ride_row(2, "Club poker night", RideStatus::Finished),
    ]);

    // Assert
    assert_eq!(state.library.rows.len(), 2);
    assert_eq!(state.library.rows[0].name, "GORBA EPIC 2026");
    assert_eq!(state.library.rows[0].status, "RUNNING");
    assert_eq!(state.library.rows[0].date, "2026-09-20");
    assert_eq!(state.library.selected, None);
}

#[test]
fn test_open_library_selected_loads_and_closes() {
    // Arrange
    let mut state = empty_state();
    state.set_library_rows(vec![ride_row(7, "GORBA EPIC 2026", RideStatus::Draft)]);
    state.open_library();
    state.library.selected = Some(0);

    // Act
    let result = state.open_library_selected();

    // Assert
    assert_eq!(result, Ok(()));
    assert!(matches!(
        state.pending_store.last(),
        Some(StoreCommand::LoadRide { id: 7 })
    ));
    assert_eq!(state.dialogs.top(), None);
}

#[test]
fn test_open_library_selected_given_no_selection_refuses() {
    // Arrange
    let mut state = empty_state();
    state.open_library();

    // Act
    let result = state.open_library_selected();

    // Assert
    assert_eq!(result, Err("Select a ride first".to_owned()));
}

#[test]
fn test_duplicate_library_selected_asks_first() {
    // Arrange
    let mut state = empty_state();
    state.set_library_rows(vec![ride_row(7, "GORBA EPIC 2026", RideStatus::Draft)]);
    state.library.selected = Some(0);

    // Act
    let asked = state.duplicate_library_selected();

    // Assert
    assert!(asked);
    assert_eq!(
        state.pending_native,
        Some(PendingNative {
            dialog: NativeDialog::Prompt,
            then: NativeTask::DuplicateRide { id: 7 },
        })
    );
}

#[test]
fn test_delete_library_selected_opens_the_typed_confirm() {
    // Arrange
    let mut state = empty_state();
    state.set_library_rows(vec![ride_row(7, "Club poker night", RideStatus::Finished)]);
    state.library.selected = Some(0);

    // Act
    let opened = state.delete_library_selected();

    // Assert
    assert!(opened);
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("delete_ride_dlg")
    );
    assert_eq!(state.library.delete_id, Some(7));
    assert!(state.library.delete_message.is_some());
    assert!(!state.delete_ride_armed());
}

#[test]
fn test_submit_delete_ride_requires_the_typed_name() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.set_library_rows(vec![ride_row(7, "Club poker night", RideStatus::Finished)]);
    state.library.selected = Some(0);
    state.delete_library_selected();
    state
        .dialogs
        .set_field("confirm_name_input", "club poker night")
        .map_err(|_| "the field".to_owned())?;

    // Act: the case differs, so the gate refuses.
    let refused = state.submit_delete_ride();
    state
        .dialogs
        .set_field("confirm_name_input", "Club poker night")
        .map_err(|_| "the field".to_owned())?;
    let armed = state.delete_ride_armed();
    let accepted = state.submit_delete_ride();

    // Assert
    assert!(refused.is_err());
    assert!(armed);
    assert_eq!(accepted, Ok(()));
    assert!(matches!(
        state.pending_store.last(),
        Some(StoreCommand::DeleteRide { id: 7, .. })
    ));
    Ok(())
}

#[test]
fn test_library_ready_and_delete_ready_follow_the_selection() {
    // Arrange
    let mut state = empty_state();
    state.set_library_rows(vec![ride_row(7, "Live", RideStatus::Running)]);

    // Act and assert
    assert!(!state.library_ready());
    state.library.selected = Some(0);
    assert!(state.library_ready());
    assert!(
        !state.library_delete_ready(),
        "a running ride is never deleted"
    );
}

// -------------------------------------------------------------- audit

#[test]
fn test_open_audit_asks_for_the_rows() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;

    // Act
    state.open_audit();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("audit_dlg")
    );
    assert!(matches!(
        state.pending_store.first(),
        Some(StoreCommand::AuditRows { .. })
    ));
    assert_eq!(state.audit.action, "All actions");
    Ok(())
}

#[test]
fn test_open_audit_given_no_ride_posts_the_notice() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.open_audit();

    // Assert
    assert_eq!(state.dialogs.top(), None);
    assert_eq!(state.notice, Some("Open a ride first".to_owned()));
}

#[test]
fn test_set_audit_rows_renders_and_filters() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;
    let at = time::OffsetDateTime::from_unix_timestamp(1_700_000_000)
        .map_err(|error| error.to_string())?;
    let rows = vec![
        AuditRow {
            at,
            action: "record_crossing".to_owned(),
            entry: "101".to_owned(),
            reason: "Ada Lovelace".to_owned(),
        },
        AuditRow {
            at,
            action: "start".to_owned(),
            entry: "201".to_owned(),
            reason: "0:00:00".to_owned(),
        },
    ];

    // Act
    state.set_audit_rows(rows);
    let all = state.filtered_audit();
    state.audit.search = "ada".to_owned();
    let searched = state.filtered_audit();
    state.audit.search.clear();
    state.audit.action = "start".to_owned();
    let action = state.filtered_audit();

    // Assert
    assert_eq!(all.len(), 2);
    assert_eq!(
        searched.len(),
        1,
        "the search reads the entry and the display name"
    );
    assert_eq!(action.len(), 1);
    assert_eq!(action[0].action, "start");
    Ok(())
}

// ----------------------------------------------------------- settings

#[test]
fn test_open_settings_seeds_the_form() -> Result<(), String> {
    // Arrange
    let (_directory, mut state) = state_with_temp_settings()?;

    // Act
    state.open_settings();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("settings_dlg")
    );
    let form = state
        .settings_form
        .as_ref()
        .ok_or_else(|| "the form".to_owned())?;
    assert_eq!(form.appearance, state.settings.appearance);
    Ok(())
}

#[test]
fn test_submit_settings_applies_saves_and_closes() -> Result<(), String> {
    // Arrange
    let (_directory, mut state) = state_with_temp_settings()?;
    state.open_settings();
    if let Some(form) = state.settings_form.as_mut() {
        form.appearance = "dark".to_owned();
        form.sound_on = false;
        form.avg_speed_kmh = 14.5;
    }

    // Act
    let result = state.submit_settings();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(state.settings.appearance, "dark");
    assert!(!state.settings.sound_on);
    assert!((state.settings.avg_speed_kmh - 14.5).abs() < f64::EPSILON);
    let (stored, warnings) = rivercrossing_store::load_settings(&state.settings_path);
    assert_eq!(stored.appearance, "dark");
    assert_eq!(warnings.len(), 0);
    assert_eq!(state.dialogs.top(), None);
    Ok(())
}

#[test]
fn test_submit_settings_given_a_bad_appearance_keeps_the_dialog() -> Result<(), String> {
    // Arrange
    let (_directory, mut state) = state_with_temp_settings()?;
    state.open_settings();
    if let Some(form) = state.settings_form.as_mut() {
        form.appearance = "sepia".to_owned();
    }

    // Act
    let result = state.submit_settings();

    // Assert
    assert_eq!(
        result,
        Err(vec!["Appearance must be system, light or dark".to_owned()])
    );
    assert_eq!(state.settings.appearance, "system");
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("settings_dlg")
    );
    assert!(state.settings_errors.is_some());
    Ok(())
}

// ---------------------------------------------------------- self-test

#[test]
fn test_open_selftest_runs_the_suite() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.open_selftest();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("selftest_dlg")
    );
    assert!(
        !state.selftest_lines.is_empty(),
        "the self-test writes report lines: {:?}",
        state.selftest_lines
    );
    assert!(
        state
            .selftest_lines
            .iter()
            .any(|line| line.contains("PASS")),
        "the report holds a PASS line: {:?}",
        state.selftest_lines
    );
}

#[test]
fn test_rerun_selftest_keeps_a_fresh_report() {
    // Arrange
    let mut state = empty_state();
    state.open_selftest();

    // Act
    state.rerun_selftest();

    // Assert
    assert!(
        !state.selftest_lines.is_empty(),
        "the rerun writes report lines: {:?}",
        state.selftest_lines
    );
}

// --------------------------------------------------------- corrections

#[test]
fn test_submit_edit_crossing_in_add_mode_records_the_crossing() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;
    state.open_add_crossing();
    for (field, value) in [
        ("plate_input", "101"),
        ("time_picker", "10:30:00"),
        ("reason_input", "missed at the line"),
    ] {
        state
            .dialogs
            .set_field(field, value)
            .map_err(|_| "the field".to_owned())?;
    }

    // Act
    let result = state.submit_edit_crossing();

    // Assert
    assert_eq!(result, Ok(()));
    let crossings = state
        .ride
        .as_ref()
        .map(|ride| ride.engine.crossings().len());
    assert_eq!(crossings, Some(2));
    assert_eq!(state.notice, Some("Crossing added".to_owned()));
    assert!(
        state
            .dialogs
            .top()
            .is_none_or(|dialog| dialog.id != "edit_crossing_dlg")
    );
    assert!(
        state
            .pending_store
            .iter()
            .any(|command| matches!(command, StoreCommand::Append { .. }))
    );
    Ok(())
}

#[test]
fn test_submit_edit_crossing_given_a_blank_reason_refuses() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;
    state.open_add_crossing();
    state
        .dialogs
        .set_field("plate_input", "101")
        .map_err(|_| "the field".to_owned())?;
    state
        .dialogs
        .set_field("time_picker", "10:30:00")
        .map_err(|_| "the field".to_owned())?;

    // Act
    let result = state.submit_edit_crossing();

    // Assert
    assert_eq!(result, Err("Enter a reason.".to_owned()));
    assert_eq!(state.notice, Some("Enter a reason.".to_owned()));
    Ok(())
}

#[test]
fn test_open_crossing_detail_renders_and_gates() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;

    // Act
    state.open_crossing_detail(&EntryId("k-ada".to_owned()), 1);

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("crossing_detail_dlg")
    );
    let detail = state
        .crossing_detail
        .as_ref()
        .ok_or_else(|| "the detail".to_owned())?;
    assert_eq!(detail.model.plate, "101");
    assert_eq!(detail.model.lap, "1");
    assert!(detail.actions.edit);
    assert!(detail.actions.delete);
    Ok(())
}

#[test]
fn test_submit_crossing_number_reassigns_the_plate() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;
    state.open_crossing_detail(&EntryId("k-ada".to_owned()), 1);
    state.open_crossing_number();
    state
        .dialogs
        .set_field("number_input", "101")
        .map_err(|_| "the field".to_owned())?;

    // Act
    let result = state.submit_crossing_number();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(state.notice, Some("Plate changed".to_owned()));
    assert!(
        state
            .pending_store
            .iter()
            .any(|command| matches!(command, StoreCommand::Append { .. }))
    );
    Ok(())
}

#[test]
fn test_submit_manual_deal_deals_a_card() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;
    state.open_manual_deal();
    for (field, value) in [("plate_input", "101"), ("reason_input", "bonus")] {
        state
            .dialogs
            .set_field(field, value)
            .map_err(|_| "the field".to_owned())?;
    }

    // Act
    let result = state.submit_manual_deal();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(state.notice, Some("Card dealt".to_owned()));
    Ok(())
}

#[test]
fn test_submit_dnf_marks_the_rider() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;
    state.open_dnf();
    for (field, value) in [("plate_input", "101"), ("reason_input", "injury")] {
        state
            .dialogs
            .set_field(field, value)
            .map_err(|_| "the field".to_owned())?;
    }

    // Act
    let result = state.submit_dnf();

    // Assert
    assert_eq!(result, Ok(()));
    assert!(state.notice.is_some_and(|notice| notice.ends_with("— DNF")));
    Ok(())
}

#[test]
fn test_submit_void_card_voids_the_credited_card() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;
    state.open_crossing_detail(&EntryId("k-ada".to_owned()), 1);
    state.open_void_card();
    state
        .dialogs
        .set_field("reason_input", "misdeal")
        .map_err(|_| "the field".to_owned())?;

    // Act
    let result = state.submit_void_card();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(state.notice, Some("Card voided".to_owned()));
    Ok(())
}

#[test]
fn test_submit_set_start_back_dates_the_gun() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;
    state.open_set_start();
    state
        .dialogs
        .set_field("start_date_picker", "2026-11-25")
        .map_err(|_| "the date".to_owned())?;
    state
        .dialogs
        .set_field("start_time_picker", "08:00:00")
        .map_err(|_| "the time".to_owned())?;

    // Act
    let result = state.submit_set_start();

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(state.notice, Some("Start time set".to_owned()));
    Ok(())
}

#[test]
fn test_delete_crossing_undoes_the_newest() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;

    // Act
    let result = state.delete_crossing(&EntryId("k-ada".to_owned()), 1);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(
        state
            .ride
            .as_ref()
            .map(|ride| ride.engine.crossings().len()),
        Some(0)
    );
    assert_eq!(state.notice, Some("Crossing deleted".to_owned()));
    Ok(())
}

// --------------------------------------------------------- quit/launch

#[test]
fn test_request_quit_with_a_running_ride_opens_the_running_dialog() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;

    // Act
    state.request_quit();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("exit_running_dlg")
    );
    assert!(state.quit_message.is_some());
    assert!(state.pending_native.is_none());
    Ok(())
}

#[test]
fn test_request_quit_without_a_running_ride_raises_the_confirm() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.request_quit();

    // Assert
    assert_eq!(
        state.pending_native,
        Some(PendingNative {
            dialog: NativeDialog::Confirm,
            then: NativeTask::Command(CommandId::AppQuit),
        })
    );
    assert_eq!(state.dialogs.top(), None);
}

#[test]
fn test_confirm_quit_and_finish_and_quit_close_the_app() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;

    // Act
    state.confirm_quit();

    // Assert
    assert!(state.close_requested);

    // Arrange
    let mut state = running_with_crossing()?;
    state.quit_message = Some("message".to_owned());

    // Act
    state.finish_and_quit();

    // Assert
    assert!(state.close_requested);
    assert_eq!(state.phase(), RidePhase::Finished);
    Ok(())
}

#[test]
fn test_apply_previous_session_opens_the_resume_dialog() {
    // Arrange
    let mut state = empty_state();
    state.set_library_rows(vec![ride_row(9, "GORBA EPIC 2026", RideStatus::Running)]);
    let session = PreviousSession {
        state: SessionState::RunningAtExit,
        ride_id: Some(9),
        ended_at: time::OffsetDateTime::from_unix_timestamp(1_700_000_000).ok(),
    };

    // Act
    state.apply_previous_session(session);

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("resume_dlg")
    );
    let message = state.launch_message.as_deref().unwrap_or_default();
    assert!(message.contains("GORBA EPIC 2026"), "{message}");
}

#[test]
fn test_resume_continue_loads_the_running_ride() {
    // Arrange
    let mut state = empty_state();
    state.previous_session = Some(PreviousSession {
        state: SessionState::RunningAtExit,
        ride_id: Some(9),
        ended_at: None,
    });
    state.reconcile_launch();
    let _ = state.dialogs.cancel();

    // Act
    state.resume_continue();

    // Assert
    assert!(matches!(
        state.pending_store.last(),
        Some(StoreCommand::LoadRide { id: 9 })
    ));
    assert_eq!(state.launch_message, None);
}

#[test]
fn test_launch_choice_opens_the_library_when_no_ride_was_running() {
    // Arrange
    let mut state = empty_state();
    state.set_library_rows(vec![ride_row(3, "Club poker night", RideStatus::Finished)]);

    // Act
    state.apply_previous_session(PreviousSession {
        state: SessionState::CleanQuit,
        ride_id: None,
        ended_at: None,
    });

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("ride_library_dlg")
    );
    assert!(state.launch_resolved);
}

#[test]
fn test_launch_choice_shows_nothing_on_a_first_launch() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.apply_previous_session(PreviousSession::fresh());

    // Assert
    assert_eq!(state.dialogs.top(), None);
    assert!(state.launch_resolved);
}

// ------------------------------------------------------------ backups

#[test]
fn test_startup_backup_queues_once_and_marks_the_hour() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.startup_backup();

    // Assert
    assert!(matches!(
        state.pending_store.first(),
        Some(StoreCommand::BackupNow)
    ));
    let hour = state.last_backup_hour;
    assert!(hour.is_some());
    let no_second = state.due_hourly_backup(time::OffsetDateTime::now_utc());
    assert!(!no_second);
}

#[test]
fn test_due_hourly_backup_fires_in_a_new_hour() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    let first = time::OffsetDateTime::from_unix_timestamp(1_700_000_000)
        .map_err(|error| error.to_string())?;
    let next = first + time::Duration::hours(1);

    // Act
    let started = state.due_hourly_backup(first);
    let again = state.due_hourly_backup(first);
    let moved_on = state.due_hourly_backup(next);

    // Assert
    assert!(started);
    assert!(!again);
    assert!(moved_on);
    assert_eq!(
        state
            .pending_store
            .iter()
            .filter(|command| matches!(command, StoreCommand::BackupNow))
            .count(),
        2
    );
    Ok(())
}

#[test]
fn test_queue_backup_queues_the_command() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.queue_backup();

    // Assert
    assert!(matches!(
        state.pending_store.first(),
        Some(StoreCommand::BackupNow)
    ));
}

#[test]
fn test_clear_ride_clears_the_active_ride() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;

    // Act
    state.clear_ride();

    // Assert
    assert!(
        state
            .pending_store
            .iter()
            .any(|command| matches!(command, StoreCommand::ClearActiveRide))
    );
    assert_eq!(state.crossing_detail, None);
    Ok(())
}

// ------------------------------------------------- results, exports, jobs

#[test]
fn test_open_results_ranks_the_finished_ride() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Finished)?;

    // Act
    state.open_results();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("results_dlg")
    );
    assert!(state.results.model.is_some());
    assert_eq!(state.results.error, None);
    Ok(())
}

#[test]
fn test_open_results_given_no_ride_shows_an_empty_board() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.open_results();

    // Assert
    assert_eq!(
        state.results.model.as_ref().map(|model| model.solo.len()),
        Some(0)
    );
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("results_dlg")
    );
}

#[test]
fn test_mark_exported_clears_the_stale_flag() -> Result<(), String> {
    // Arrange
    let mut state = running_with_crossing()?;
    let start = epoch_start();
    if let Some(ride) = state.ride.as_mut() {
        ride.engine
            .add_crossing_at(
                "101",
                start + time::Duration::hours(3),
                "missed at the line",
            )
            .map_err(|error| error.to_string())?;
    }
    state.open_results();
    if let Some(ride) = state.ride.as_ref() {
        let watermark = ride.engine.events().len();
        state.results.watermark.mark_exported(watermark);
    }
    if let Some(ride) = state.ride.as_mut() {
        ride.engine
            .add_crossing_at(
                "101",
                start + time::Duration::hours(4),
                "correction after export",
            )
            .map_err(|error| error.to_string())?;
    }

    // Act
    state.refresh_results();
    let stale_after_correction = state
        .results
        .model
        .as_ref()
        .is_some_and(|model| model.stale);
    state.mark_exported();

    // Assert
    assert!(stale_after_correction);
    assert_eq!(
        state
            .results
            .model
            .as_ref()
            .is_some_and(|model| model.stale),
        false
    );
    Ok(())
}

#[test]
fn test_queue_export_composes_the_job() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Finished)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("results.html");

    // Act
    state.queue_export(ExportKind::Html, &path)?;

    // Assert
    let job = state.pending_export.as_ref().ok_or("the job is queued")?;
    assert_eq!(job.kind, ExportKind::Html);
    assert_eq!(job.path, path);
    Ok(())
}

#[test]
fn test_queue_export_given_no_ride_refuses() {
    // Arrange
    let mut state = empty_state();

    // Act
    let result = state.queue_export(ExportKind::Csv, std::path::Path::new("x.csv"));

    // Assert
    assert_eq!(result, Err("Open a ride first".to_owned()));
    assert!(state.pending_export.is_none());
}

#[test]
fn test_record_export_marks_presence_and_the_preview_target() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Finished)?;
    let path = std::path::Path::new("/tmp/results.html");

    // Act
    state.record_export(ExportKind::Html, path);

    // Assert
    assert!(state.exports.html);
    assert_eq!(
        state.preview_target(ExportKind::Html),
        Ok(path.to_path_buf())
    );
    assert_eq!(
        state.preview_target(ExportKind::PosterHtml),
        Err("Export the file first".to_owned())
    );
    assert_eq!(state.notice, Some("Exported /tmp/results.html".to_owned()));
    Ok(())
}

#[test]
fn test_open_publish_prefills_the_form_from_the_settings_and_ride() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Finished)?;
    state.settings.wp_url = "https://example.test".to_owned();
    state.settings.wp_username = "organizer".to_owned();

    // Act
    state.open_publish();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("publish_wordpress_dlg")
    );
    let form = state.publish_form();
    assert_eq!(form.base_url, "https://example.test");
    assert_eq!(form.username, "organizer");
    assert_eq!(form.kind, "full");
    assert!(form.title.contains("Full Results"));
    assert!(form.slug.contains("full-results"));
    Ok(())
}

#[test]
fn test_submit_publish_given_an_empty_form_keeps_the_dialog() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Finished)?;
    state.open_publish();

    // Act
    let result = state.submit_publish();

    // Assert
    assert!(result.is_err());
    assert!(state.publish_errors.is_some());
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("publish_wordpress_dlg")
    );
    assert!(state.pending_publish.is_none());
    Ok(())
}

#[test]
fn test_submit_publish_given_a_valid_form_persists_and_queues_the_job() -> Result<(), String> {
    // Arrange
    let (_directory, mut state) = state_with_temp_settings()?;
    state.ride = loaded_ride(RidePhase::Finished)?;
    state.open_publish();
    for (field, value) in [
        ("wp_url_input", "https://example.test"),
        ("wp_username_input", "organizer"),
        ("wp_password_input", "abcd efgh"),
    ] {
        state
            .dialogs
            .set_field(field, value)
            .map_err(|_| "the field".to_owned())?;
    }

    // Act
    state.submit_publish()?;

    // Assert
    assert_eq!(state.settings.wp_url, "https://example.test");
    assert_eq!(state.settings.wp_username, "organizer");
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("publish_running_dlg")
    );
    assert!(state.pending_publish.is_some());
    let stored = rivercrossing_store::load_settings(&state.settings_path).0;
    assert_eq!(stored.wp_url, "https://example.test");
    Ok(())
}

#[test]
fn test_retry_publish_rebuilds_the_kept_form() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Finished)?;
    state.open_publish();
    for (field, value) in [
        ("wp_url_input", "https://example.test"),
        ("wp_username_input", "organizer"),
        ("wp_password_input", "abcd efgh"),
    ] {
        state
            .dialogs
            .set_field(field, value)
            .map_err(|_| "the field".to_owned())?;
    }
    state.submit_publish()?;
    state.pending_publish = None;

    // Act
    state.retry_publish()?;

    // Assert
    assert!(state.pending_publish.is_some());
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("publish_running_dlg")
    );
    Ok(())
}

#[test]
fn test_request_publish_cancel_sets_the_flag_and_the_status() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.request_publish_cancel();

    // Assert
    assert!(state.publish_cancel);
    assert_eq!(state.publish_status, "Cancelling…");
}

#[test]
fn test_open_simulation_prefills_the_saved_settings() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Draft)?;
    state.settings.sim_riders = 120;
    state.settings.sim_teams = 20;
    state.settings.sim_laps = 3;

    // Act
    state.open_simulation();

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("simulation_dlg")
    );
    assert_eq!(state.dialogs.field("riders_spin"), Some("120"));
    let (riders, teams, _solo, laps, _interval, _a, _b, _c) = state.simulation_counts();
    assert_eq!((riders, teams, laps), (120, 20, 3));
    Ok(())
}

#[test]
fn test_check_simulation_fills_the_infobar() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Draft)?;
    state.open_simulation();

    // Act
    state.check_simulation();

    // Assert
    assert!(
        state
            .sim_message
            .as_deref()
            .is_some_and(|message| message.contains("Team riders = teams"))
    );
    Ok(())
}

#[test]
fn test_submit_simulation_given_bad_laps_keeps_the_dialog() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Draft)?;
    state.open_simulation();
    state
        .dialogs
        .set_field("laps_spin", "0")
        .map_err(|_| "the field".to_owned())?;

    // Act
    let result = state.submit_simulation();

    // Assert
    assert_eq!(result, Err("laps must be at least 1".to_owned()));
    assert!(state.sim_message.is_some());
    assert!(state.pending_sim.is_none());
    Ok(())
}

#[test]
fn test_submit_simulation_given_valid_counts_persists_and_queues_the_run() -> Result<(), String> {
    // Arrange
    let (_directory, mut state) = state_with_temp_settings()?;
    state.ride = loaded_ride(RidePhase::Draft)?;
    state.open_simulation();
    state
        .dialogs
        .set_field("laps_spin", "2")
        .map_err(|_| "the field".to_owned())?;
    state
        .dialogs
        .set_field("interval_spin", "15")
        .map_err(|_| "the field".to_owned())?;

    // Act
    state.submit_simulation()?;

    // Assert
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("sim_running_dlg")
    );
    assert!(state.pending_sim.is_some());
    let stored = rivercrossing_store::load_settings(&state.settings_path).0;
    assert_eq!((stored.sim_laps, stored.sim_interval), (2, 15));
    Ok(())
}

#[test]
fn test_generate_sim_riders_fills_the_draft_roster() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Draft)?;
    state.open_simulation();
    for (field, value) in [
        ("riders_spin", "6"),
        ("teams_spin", "1"),
        ("solo_spin", "2"),
    ] {
        state
            .dialogs
            .set_field(field, value)
            .map_err(|_| "the field".to_owned())?;
    }

    // Act
    state.generate_sim_riders()?;

    // Assert
    let roster = state.ride.as_ref().map(|ride| ride.roster.entries().len());
    assert!(roster.is_some_and(|entries| entries >= 2));
    assert_eq!(state.notice, Some("Generated 6 riders".to_owned()));
    Ok(())
}

#[test]
fn test_finish_simulation_persists_the_new_events_and_closes() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Draft)?;
    state.open_simulation();
    let mut ride = state.ride.take().ok_or("the ride")?;
    let pre_events = ride.engine.events().len();
    let start = epoch_start();
    ride.engine
        .start(Some(start))
        .map_err(|error| error.to_string())?;
    let _ = ride
        .engine
        .record_crossing("101", Some(start + time::Duration::hours(1)));
    let outcome = simulator::SimOutcome {
        cancelled: false,
        recorded: 1,
        blocked: None,
    };

    // Act
    state.finish_simulation(ride, outcome, pre_events);

    // Assert
    assert!(state.ride.is_some());
    assert_eq!(state.dialogs.top(), None);
    assert_eq!(state.notice, Some("Simulated 1 crossings".to_owned()));
    assert!(
        state
            .pending_store
            .iter()
            .any(|command| matches!(command, StoreCommand::Append { .. }))
    );
    Ok(())
}

#[test]
fn test_request_sim_cancel_sets_the_flag_and_the_status() {
    // Arrange
    let mut state = empty_state();

    // Act
    state.request_sim_cancel();

    // Assert
    assert!(state.sim_cancel);
    assert_eq!(state.sim_status, "Cancelling…");
}

#[test]
fn test_set_publish_kind_reseeds_the_title_and_slug() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.ride = loaded_ride(RidePhase::Finished)?;
    state.open_publish();

    // Act
    state.set_publish_kind("podium");

    // Assert
    let form = state.publish_form();
    assert_eq!(form.kind, "podium");
    assert!(form.title.contains("Podium Results"), "{}", form.title);
    assert!(form.slug.contains("podium-results"), "{}", form.slug);
    Ok(())
}

#[test]
fn test_persist_sim_settings_saves_the_fields_without_a_run() -> Result<(), String> {
    // Arrange
    let (_directory, mut state) = state_with_temp_settings()?;
    state.open_simulation();
    for (field, value) in [("riders_spin", "77"), ("laps_spin", "4")] {
        state
            .dialogs
            .set_field(field, value)
            .map_err(|_| "the field".to_owned())?;
    }

    // Act
    state.persist_sim_settings();

    // Assert
    let stored = rivercrossing_store::load_settings(&state.settings_path).0;
    assert_eq!((stored.sim_riders, stored.sim_laps), (77, 4));
    assert!(state.pending_sim.is_none());
    Ok(())
}
