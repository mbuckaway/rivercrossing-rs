// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the app state: the phase and gate projections, the status
//! notice, the settings write, the new-ride seed and the action execution.

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
use crate::ui::dialogs::stack::Focus;
use crate::ui::presenters::console::RIDE_STARTED;
use crate::ui::sound::Cue;

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
