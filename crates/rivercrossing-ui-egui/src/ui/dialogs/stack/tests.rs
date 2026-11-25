// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the dialog stack: open, close, Esc, Enter, Tab and focus return.

use std::collections::BTreeMap;

use super::{DialogError, DialogEvent, DialogStack, Focus, Key, OpenDialog};
use crate::ui::dialogs::spec::Button;

/// Route the key to the stack `count` times and return the top dialog's focus after each press.
fn press(
    stack: &mut DialogStack,
    key: Key,
    count: usize,
) -> Result<Vec<Option<Focus>>, DialogError> {
    let mut focuses = Vec::with_capacity(count);
    for _ in 0..count {
        assert_eq!(stack.key(key)?, Some(DialogEvent::FocusChanged));
        focuses.push(stack.top().map(|dialog| dialog.focus.clone()));
    }
    Ok(focuses)
}

/// Build a stack whose only dialog carries an id that the registry does not hold.
fn stack_with_an_unknown_dialog() -> DialogStack {
    DialogStack {
        open: vec![OpenDialog {
            id: "no_such_dlg",
            opener: None,
            fields: BTreeMap::new(),
            focus: Focus::Button(Button::Ok),
            confirmed_button: None,
        }],
    }
}

#[test]
fn test_open_sets_the_initial_focus_to_the_first_field() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();

    // Act
    stack.open("ride_setup_dlg", Some("main_frame"))?;

    // Assert
    assert_eq!(
        stack.top().map(|dialog| dialog.focus.clone()),
        Some(Focus::Field("name_input"))
    );
    Ok(())
}

#[test]
fn test_open_without_a_field_focuses_the_primary_button() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();

    // Act
    stack.open("resume_dlg", None)?;

    // Assert
    assert_eq!(
        stack.top().map(|dialog| dialog.focus.clone()),
        Some(Focus::Button(Button::Custom("continue_btn")))
    );
    Ok(())
}

#[test]
fn test_a_destructive_dialog_starts_on_cancel() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();

    // Act
    stack.open("delete_ride_dlg", Some("ride_library_dlg"))?;

    // Assert
    assert_eq!(
        stack.top().map(|dialog| dialog.focus.clone()),
        Some(Focus::Button(Button::Cancel))
    );
    Ok(())
}

#[test]
fn test_esc_cancels_a_non_destructive_dialog_and_reports_the_opener() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("ride_setup_dlg", Some("main_frame"))?;

    // Act
    let event = stack.key(Key::Escape);

    // Assert
    assert_eq!(
        event,
        Ok(Some(DialogEvent::Cancelled {
            id: "ride_setup_dlg",
            opener: Some("main_frame"),
        }))
    );
    assert_eq!(stack.top(), None);
    Ok(())
}

#[test]
fn test_enter_activates_the_default_button() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("ride_setup_dlg", Some("main_frame"))?;

    // Act
    let event = stack.key(Key::Enter);

    // Assert
    assert_eq!(
        event,
        Ok(Some(DialogEvent::Confirmed {
            id: "ride_setup_dlg",
            button: Button::Ok,
            opener: Some("main_frame"),
        }))
    );
    Ok(())
}

#[test]
fn test_destructive_confirm_defaults_to_cancel() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("delete_ride_dlg", Some("ride_library_dlg"))?;

    // Act
    let event = stack.confirm();

    // Assert
    assert_eq!(
        event,
        Ok(DialogEvent::Confirmed {
            id: "delete_ride_dlg",
            button: Button::Cancel,
            opener: Some("ride_library_dlg"),
        })
    );
    Ok(())
}

#[test]
fn test_focus_is_returned_to_the_opener_on_close() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("start_blocked_dlg", Some("start_btn"))?;

    // Act
    let event = stack.confirm();

    // Assert
    assert_eq!(
        event,
        Ok(DialogEvent::Confirmed {
            id: "start_blocked_dlg",
            button: Button::Ok,
            opener: Some("start_btn"),
        })
    );
    Ok(())
}

#[test]
fn test_nested_dialogs_close_in_order() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("ride_setup_dlg", Some("main_frame"))?;
    stack.open("set_start_dlg", Some("ride_setup_dlg"))?;

    // Act
    let event = stack.cancel();

    // Assert
    assert_eq!(
        event,
        Ok(DialogEvent::Cancelled {
            id: "set_start_dlg",
            opener: Some("ride_setup_dlg"),
        })
    );
    assert_eq!(stack.top().map(|dialog| dialog.id), Some("ride_setup_dlg"));
    Ok(())
}

#[test]
fn test_tab_and_shift_tab_cycle_inside_the_dialog_only() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("ride_setup_dlg", Some("main_frame"))?;
    stack.open("start_blocked_dlg", Some("ride_setup_dlg"))?;

    // Act
    let forward = press(&mut stack, Key::Tab, 3)?;
    let backward = press(&mut stack, Key::ShiftTab, 2)?;

    // Assert
    let cancel = Some(Focus::Button(Button::Cancel));
    let ok = Some(Focus::Button(Button::Ok));
    assert_eq!(forward, vec![cancel.clone(), ok.clone(), cancel.clone()]);
    assert_eq!(backward, vec![ok, cancel]);
    assert_eq!(
        stack.top().map(|dialog| dialog.id),
        Some("start_blocked_dlg")
    );
    Ok(())
}

#[test]
fn test_tab_on_a_single_button_dialog_keeps_the_button() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("about_dlg", None)?;

    // Act
    let focuses = press(&mut stack, Key::Tab, 1)?;

    // Assert
    assert_eq!(focuses, vec![Some(Focus::Button(Button::Close))]);
    assert_eq!(stack.top().map(|dialog| dialog.id), Some("about_dlg"));
    Ok(())
}

#[test]
fn test_tab_returns_a_focus_changed_event() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("start_blocked_dlg", None)?;

    // Act
    let event = stack.key(Key::Tab);

    // Assert
    assert_eq!(event, Ok(Some(DialogEvent::FocusChanged)));
    Ok(())
}

#[test]
fn test_set_field_writes_and_reads_the_top_dialog() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("ride_setup_dlg", None)?;

    // Act
    stack.set_field("name_input", "Club poker night")?;

    // Assert
    assert_eq!(stack.field("name_input"), Some("Club poker night"));
    Ok(())
}

#[test]
fn test_set_field_writes_a_field_that_is_not_the_initial_focus() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("delete_ride_dlg", None)?;

    // Act
    stack.set_field("confirm_name_input", "Club poker night")?;

    // Assert
    assert_eq!(stack.field("confirm_name_input"), Some("Club poker night"));
    Ok(())
}

#[test]
fn test_set_field_given_an_unknown_field_errors() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("ride_setup_dlg", None)?;

    // Act
    let result = stack.set_field("bogus_field", "x");

    // Assert
    assert_eq!(result, Err(DialogError::UnknownField));
    Ok(())
}

#[test]
fn test_set_field_given_a_field_of_another_dialog_errors() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("ride_setup_dlg", None)?;

    // Act
    let result = stack.set_field("plate_input", "R1");

    // Assert
    assert_eq!(result, Err(DialogError::UnknownField));
    Ok(())
}

#[test]
fn test_set_field_given_a_control_that_is_not_settable_errors() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("ride_setup_dlg", None)?;

    // Act
    let result = stack.set_field("setup_infobar", "x");

    // Assert
    assert_eq!(result, Err(DialogError::UnknownField));
    Ok(())
}

#[test]
fn test_key_on_an_empty_stack_errors() {
    // Arrange
    let mut stack = DialogStack::default();

    // Act
    let result = stack.key(Key::Tab);

    // Assert
    assert_eq!(result, Err(DialogError::NotOpen));
}

#[test]
fn test_cancel_on_an_empty_stack_errors() {
    // Arrange
    let mut stack = DialogStack::default();

    // Act
    let result = stack.cancel();

    // Assert
    assert_eq!(result, Err(DialogError::NotOpen));
}

#[test]
fn test_confirm_on_an_empty_stack_errors() {
    // Arrange
    let mut stack = DialogStack::default();

    // Act
    let result = stack.confirm();

    // Assert
    assert_eq!(result, Err(DialogError::NotOpen));
}

#[test]
fn test_set_field_on_an_empty_stack_errors() {
    // Arrange
    let mut stack = DialogStack::default();

    // Act
    let result = stack.set_field("name_input", "x");

    // Assert
    assert_eq!(result, Err(DialogError::NotOpen));
}

#[test]
fn test_field_on_an_empty_stack_returns_none() {
    // Arrange
    let stack = DialogStack::default();

    // Act
    let found = stack.field("name_input");

    // Assert
    assert_eq!(found, None);
}

#[test]
fn test_field_given_an_unset_field_returns_none() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("ride_setup_dlg", None)?;

    // Act
    let found = stack.field("name_input");

    // Assert
    assert_eq!(found, None);
    Ok(())
}

#[test]
fn test_open_given_an_unknown_dialog_errors() {
    // Arrange
    let mut stack = DialogStack::default();

    // Act
    let result = stack.open("no_such_dlg", None);

    // Assert
    assert_eq!(result, Err(DialogError::UnknownDialog));
    assert_eq!(stack.top(), None);
}

#[test]
fn test_open_given_a_window_that_is_not_a_dialog_errors() {
    // Arrange
    let mut stack = DialogStack::default();

    // Act
    let result = stack.open("main_frame", None);

    // Assert
    assert_eq!(result, Err(DialogError::UnknownDialog));
    assert_eq!(stack.top(), None);
}

#[test]
fn test_a_fresh_dialog_has_no_confirmed_button() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("start_blocked_dlg", None)?;

    // Act
    let confirmed = stack.top().map(|dialog| dialog.confirmed_button);

    // Assert
    assert_eq!(confirmed, Some(None));
    Ok(())
}

#[test]
fn test_confirm_given_a_dialog_missing_from_the_registry_errors() {
    // Arrange
    let mut stack = stack_with_an_unknown_dialog();

    // Act
    let result = stack.confirm();

    // Assert
    assert_eq!(result, Err(DialogError::UnknownDialog));
}

#[test]
fn test_tab_given_a_dialog_missing_from_the_registry_errors() {
    // Arrange
    let mut stack = stack_with_an_unknown_dialog();

    // Act
    let result = stack.key(Key::Tab);

    // Assert
    assert_eq!(result, Err(DialogError::UnknownDialog));
}

#[test]
fn test_is_empty_with_no_dialog_open() {
    // Arrange
    let stack = DialogStack::default();

    // Act
    let state = (stack.is_empty(), stack.top().is_none());

    // Assert
    assert_eq!(state, (true, true));
}

#[test]
fn test_is_empty_with_a_dialog_open() -> Result<(), DialogError> {
    // Arrange
    let mut stack = DialogStack::default();
    stack.open("about_dlg", None)?;

    // Act
    let state = (stack.is_empty(), stack.top().is_none());

    // Assert
    assert_eq!(stack.top().map(|dialog| dialog.id), Some("about_dlg"));
    assert_eq!(state, (false, false));
    Ok(())
}

#[test]
fn test_activate_reports_the_clicked_button_not_the_default() -> Result<(), String> {
    // Arrange
    let mut stack = DialogStack::default();
    stack
        .open("resume_dlg", None)
        .map_err(|_| "the resume dialog refuses to open".to_owned())?;

    // Act
    let event = stack
        .activate(Button::Custom("library_btn"))
        .map_err(|_| "the dialog was not open".to_owned())?;

    // Assert
    assert_eq!(
        event,
        DialogEvent::Confirmed {
            id: "resume_dlg",
            button: Button::Custom("library_btn"),
            opener: None,
        }
    );
    assert!(stack.is_empty());
    Ok(())
}

#[test]
fn test_activate_given_no_dialog_errors() {
    // Arrange
    let mut stack = DialogStack::default();

    // Act
    let event = stack.activate(Button::Ok);

    // Assert
    assert_eq!(event, Err(DialogError::NotOpen));
}
