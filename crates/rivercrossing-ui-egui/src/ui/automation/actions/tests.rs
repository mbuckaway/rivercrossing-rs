// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the `ui.*` action helpers: the action dispatch, the qualified field reads and
//! writes, the dialog trio and the click selector.

use pretty_assertions::assert_eq;
use rstest::rstest;
use serde_json::{Value, json};

use super::{action_target, command, dialog, inspect, set};
use crate::ui::action::spec::{CommandId, NativeDialog};
use crate::ui::state::{AppState, NativeTask};

// ------------------------------------------------------------- fixtures

/// Return an empty app state over the default settings.
fn empty_state() -> AppState {
    AppState::blank()
}

/// Return the empty state with the ride setup dialog open.
fn state_with_setup_dialog() -> Result<AppState, String> {
    let mut state = empty_state();
    state
        .dialogs
        .open("ride_setup_dlg", None)
        .map_err(|_| "the setup dialog refuses to open".to_owned())?;
    Ok(state)
}

// ------------------------------------------------------------- ui.command

#[test]
fn test_command_runs_a_built_action() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();

    // Act
    let reply = command(&mut state, "mi_new_ride")?;

    // Assert
    assert_eq!(reply, json!({"ok": true}));
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("ride_setup_dlg")
    );
    Ok(())
}

#[test]
fn test_command_app_quit_answers_bye() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();

    // Act
    let reply = command(&mut state, "app_quit")?;

    // Assert
    assert_eq!(reply, json!({"bye": true}));
    assert!(state.dialogs.is_empty());
    assert_eq!(state.pending_command, None);
    Ok(())
}

#[test]
fn test_command_given_an_unknown_name_errors() {
    // Arrange
    let mut state = empty_state();

    // Act
    let reply = command(&mut state, "nope");

    // Assert
    assert_eq!(reply, Err("unknown command `nope`".to_owned()));
}

#[test]
fn test_command_opens_the_standings_window() {
    // Arrange
    let mut state = empty_state();

    // Act
    let reply = command(&mut state, "mi_standings");

    // Assert
    assert_eq!(reply, Ok(json!({ "ok": true })));
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("results_dlg")
    );
}

#[test]
fn test_command_given_a_disabled_action_errors() {
    // Arrange
    let mut state = empty_state();

    // Act
    let reply = command(&mut state, "mi_finish_ride");

    // Assert
    assert_eq!(
        reply,
        Err("command `mi_finish_ride` is disabled in this state".to_owned())
    );
    assert_eq!(state.pending_native, None);
}

// ------------------------------------------------------------- ui.inspect

#[test]
fn test_inspect_reads_the_top_dialog_field() -> Result<(), String> {
    // Arrange
    let mut state = state_with_setup_dialog()?;
    state
        .dialogs
        .set_field("name_input", "GORBA EPIC 2026")
        .map_err(|_| "the setup dialog refuses the field".to_owned())?;

    // Act
    let reply = inspect(&state, &json!({"field": "name_input"}))?;

    // Assert
    assert_eq!(reply, json!({"value": "GORBA EPIC 2026"}));
    Ok(())
}

#[test]
fn test_inspect_returns_null_for_an_unset_field() -> Result<(), String> {
    // Arrange
    let state = state_with_setup_dialog()?;

    // Act
    let reply = inspect(&state, &json!({"field": "venue_input"}))?;

    // Assert
    assert_eq!(reply, json!({"value": null}));
    Ok(())
}

#[test]
fn test_inspect_given_an_unknown_field_errors() -> Result<(), String> {
    // Arrange
    let state = state_with_setup_dialog()?;

    // Act
    let reply = inspect(&state, &json!({"field": "nope"}));

    // Assert
    assert_eq!(reply, Err("unknown field `nope`".to_owned()));
    Ok(())
}

#[test]
fn test_inspect_given_no_dialog_errors() {
    // Arrange
    let state = empty_state();

    // Act
    let reply = inspect(&state, &json!({"field": "name_input"}));

    // Assert
    assert_eq!(reply, Err("no dialog is open".to_owned()));
}

#[test]
fn test_inspect_given_a_window_and_no_dialog_errors() {
    // Arrange
    let state = empty_state();

    // Act
    let reply = inspect(
        &state,
        &json!({"field": "name_input", "window": "ride_setup_dlg"}),
    );

    // Assert
    assert_eq!(reply, Err("no dialog is open".to_owned()));
}

#[test]
fn test_inspect_given_a_non_string_field_errors() -> Result<(), String> {
    // Arrange
    let state = state_with_setup_dialog()?;

    // Act
    let reply = inspect(&state, &json!({"field": 7}));

    // Assert
    assert_eq!(reply, Err("ui.inspect needs a string `field`".to_owned()));
    Ok(())
}

#[test]
fn test_inspect_given_a_non_string_window_errors() -> Result<(), String> {
    // Arrange
    let state = state_with_setup_dialog()?;

    // Act
    let reply = inspect(&state, &json!({"field": "name_input", "window": 7}));

    // Assert
    assert_eq!(reply, Err("ui.inspect needs a string `window`".to_owned()));
    Ok(())
}

// ----------------------------------------------------------------- ui.set

#[test]
fn test_set_writes_the_top_dialog_field() -> Result<(), String> {
    // Arrange
    let mut state = state_with_setup_dialog()?;

    // Act
    let reply = set(
        &mut state,
        &json!({"field": "name_input", "value": "GORBA EPIC 2026"}),
    )?;

    // Assert
    assert_eq!(reply, json!({"ok": true}));
    assert_eq!(state.dialogs.field("name_input"), Some("GORBA EPIC 2026"));
    Ok(())
}

#[test]
fn test_set_given_a_window_name_targets_that_dialog() -> Result<(), String> {
    // Arrange
    let mut state = state_with_setup_dialog()?;

    // Act
    let reply = set(
        &mut state,
        &json!({
            "field": "venue_input",
            "value": "Sea to Sky Gondola",
            "window": "ride_setup_dlg",
        }),
    )?;

    // Assert
    assert_eq!(reply, json!({"ok": true}));
    assert_eq!(
        state.dialogs.field("venue_input"),
        Some("Sea to Sky Gondola")
    );
    Ok(())
}

#[test]
fn test_set_given_a_non_string_field_errors() -> Result<(), String> {
    // Arrange
    let mut state = state_with_setup_dialog()?;

    // Act
    let reply = set(&mut state, &json!({"field": 7, "value": "x"}));

    // Assert
    assert_eq!(reply, Err("ui.set needs a string `field`".to_owned()));
    Ok(())
}

#[test]
fn test_set_given_another_window_errors() -> Result<(), String> {
    // Arrange
    let mut state = state_with_setup_dialog()?;

    // Act
    let reply = set(
        &mut state,
        &json!({"field": "name_input", "value": "x", "window": "about_dlg"}),
    );

    // Assert
    assert_eq!(reply, Err("no dialog `about_dlg` is open".to_owned()));
    assert_eq!(state.dialogs.field("name_input"), None);
    Ok(())
}

#[test]
fn test_set_given_an_unknown_field_errors() -> Result<(), String> {
    // Arrange
    let mut state = state_with_setup_dialog()?;

    // Act
    let reply = set(&mut state, &json!({"field": "nope", "value": "x"}));

    // Assert
    assert_eq!(reply, Err("unknown field `nope`".to_owned()));
    Ok(())
}

#[test]
fn test_set_given_no_dialog_errors() {
    // Arrange
    let mut state = empty_state();

    // Act
    let reply = set(&mut state, &json!({"field": "name_input", "value": "x"}));

    // Assert
    assert_eq!(reply, Err("no dialog is open".to_owned()));
}

#[test]
fn test_set_given_a_missing_value_errors() -> Result<(), String> {
    // Arrange
    let mut state = state_with_setup_dialog()?;

    // Act
    let reply = set(&mut state, &json!({"field": "name_input"}));

    // Assert
    assert_eq!(reply, Err("ui.set needs a string `value`".to_owned()));
    Ok(())
}

// ------------------------------------------------------------- ui.dialog.*

#[rstest]
#[case("ui.dialog.confirm", json!({"event": "confirmed", "button": "Close", "opener": null}))]
#[case("ui.dialog.cancel", json!({"event": "cancelled", "button": "Close", "opener": null}))]
fn test_dialog_confirm_and_cancel_return_the_event(
    #[case] method: &str,
    #[case] expected: Value,
) -> Result<(), String> {
    // Arrange: a plain message dialog, not a form whose OK submits.
    let mut state = empty_state();
    state
        .dialogs
        .open("about_dlg", None)
        .map_err(|_| "the about dialog refuses to open".to_owned())?;

    // Act
    let reply = dialog(&mut state, method, &json!({}))?;

    // Assert
    assert_eq!(reply, expected);
    assert!(state.dialogs.is_empty());
    Ok(())
}

#[test]
fn test_dialog_cancel_reports_the_dialog_cancel_button() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state
        .dialogs
        .open("about_dlg", None)
        .map_err(|_| "the about dialog refuses to open".to_owned())?;

    // Act
    let reply = dialog(&mut state, "ui.dialog.cancel", &json!({}))?;

    // Assert
    assert_eq!(
        reply,
        json!({"event": "cancelled", "button": "Close", "opener": null})
    );
    Ok(())
}

#[test]
fn test_dialog_set_writes_the_top_dialog_field() -> Result<(), String> {
    // Arrange
    let mut state = state_with_setup_dialog()?;

    // Act
    let reply = dialog(
        &mut state,
        "ui.dialog.set",
        &json!({"field": "name_input", "value": "GORBA EPIC 2026"}),
    )?;

    // Assert
    assert_eq!(reply, json!({"ok": true}));
    assert_eq!(state.dialogs.field("name_input"), Some("GORBA EPIC 2026"));
    Ok(())
}

#[test]
fn test_dialog_given_an_unknown_method_errors() -> Result<(), String> {
    // Arrange
    let mut state = state_with_setup_dialog()?;

    // Act
    let reply = dialog(&mut state, "ui.dialog.nope", &json!({}));

    // Assert
    assert_eq!(reply, Err("unknown method `ui.dialog.nope`".to_owned()));
    Ok(())
}

#[test]
fn test_dialog_confirm_given_no_dialog_errors() {
    // Arrange
    let mut state = empty_state();

    // Act
    let reply = dialog(&mut state, "ui.dialog.confirm", &json!({}));

    // Assert
    assert_eq!(reply, Err("no dialog is open".to_owned()));
}

#[test]
fn test_dialog_cancel_given_no_dialog_errors() {
    // Arrange
    let mut state = empty_state();

    // Act
    let reply = dialog(&mut state, "ui.dialog.cancel", &json!({}));

    // Assert
    assert_eq!(reply, Err("no dialog is open".to_owned()));
}

// ---------------------------------------------------------- action_target

#[rstest]
#[case("mi_start_ride", Some(("Button", "Start Ride")))]
#[case("mi_stop_ride", Some(("Button", "Stop Ride")))]
#[case("mi_undo_crossing", Some(("Button", "Undo")))]
#[case("mi_backup_now", None)]
#[case("record", None)]
fn test_action_target_maps_the_console_buttons_and_rejects_the_rest(
    #[case] action: &str,
    #[case] expected: Option<(&'static str, &'static str)>,
) {
    // Act
    let target = action_target(action);

    // Assert
    assert_eq!(target, expected);
}

// ------------------------------------------------------ native confirms

#[test]
fn test_dialog_confirm_queues_a_pending_native_command() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state
        .act(crate::ui::action::id::MI_BACKUP_NOW)
        .map_err(|error| format!("{error:?}"))?;
    state.pending_command = None;
    state.pending_native = Some(crate::ui::state::PendingNative {
        dialog: NativeDialog::Danger,
        then: NativeTask::Command(CommandId::UndoCrossing),
    });

    // Act
    let reply = dialog(&mut state, "ui.dialog.confirm", &json!({}))?;

    // Assert
    assert_eq!(
        reply,
        json!({"event": "confirmed", "button": "OK", "opener": null})
    );
    assert_eq!(state.pending_native, None);
    assert_eq!(state.pending_command, Some(CommandId::UndoCrossing));
    Ok(())
}

#[test]
fn test_dialog_cancel_clears_a_pending_native_without_a_command() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state.pending_native = Some(crate::ui::state::PendingNative {
        dialog: NativeDialog::Danger,
        then: NativeTask::Command(CommandId::UndoCrossing),
    });

    // Act
    let reply = dialog(&mut state, "ui.dialog.cancel", &json!({}))?;

    // Assert
    assert_eq!(
        reply,
        json!({"event": "cancelled", "button": "Cancel", "opener": null})
    );
    assert_eq!(state.pending_native, None);
    assert_eq!(state.pending_command, None);
    Ok(())
}

#[test]
fn test_dialog_confirm_submits_the_setup_form() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state
        .dialogs
        .open("ride_setup_dlg", None)
        .map_err(|_| "the setup dialog".to_owned())?;
    for (field, value) in [
        ("name_input", "GORBA EPIC 2026"),
        ("date_picker", "2026-11-25"),
        ("start_time_picker", "08:00"),
        ("venue_input", "Sea to Sky Gondola"),
        ("lap_km_spin", "8.0"),
        ("organizer_input", "GORBA"),
        ("scorer_input", "K. Singh"),
        ("duration_input", "6:00"),
        ("min_lap_input", "20:00"),
    ] {
        state
            .dialogs
            .set_field(field, value)
            .map_err(|_| "the setup field".to_owned())?;
    }

    // Act
    let reply = dialog(&mut state, "ui.dialog.confirm", &json!({}))?;

    // Assert
    assert_eq!(
        reply,
        json!({"event": "confirmed", "button": "OK", "opener": null})
    );
    assert!(state.pending_create.is_some());
    assert!(state.dialogs.is_empty());
    Ok(())
}

#[test]
fn test_dialog_confirm_given_a_refused_form_reports_the_message() -> Result<(), String> {
    // Arrange
    let mut state = empty_state();
    state
        .dialogs
        .open("ride_setup_dlg", None)
        .map_err(|_| "the setup dialog".to_owned())?;

    // Act
    let reply = dialog(&mut state, "ui.dialog.confirm", &json!({}));

    // Assert
    let message = reply
        .err()
        .ok_or_else(|| "the blank form is refused".to_owned())?;
    assert!(
        message.contains("Date is empty"),
        "the refusal names the first blank field: {message}"
    );
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("ride_setup_dlg")
    );
    assert!(state.pending_create.is_none());
    Ok(())
}
