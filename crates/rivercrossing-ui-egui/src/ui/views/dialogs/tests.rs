// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the dialog host's button roles and close mapping.

use craft::ui::contract::LeastWidth;
use craft::ui::runtime::overlay::CloseReason;
use craft::ui::sections::DialogRole;

use super::{Activation, activation, button_role, least_width};
use crate::ui::dialogs::spec::{Button, DialogSpec, spec};

/// Return the frozen contract of *id*; the test fixtures name frozen dialogs only.
fn dialog(id: &str) -> &'static DialogSpec {
    spec(id).expect("the fixture names a frozen dialog")
}

#[test]
fn test_button_role_given_the_enter_default_returns_accept() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let role = button_role(dialog, Button::Ok);

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_the_cancel_button_returns_cancel() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let role = button_role(dialog, Button::Cancel);

    // Assert
    assert_eq!(role, DialogRole::Cancel);
}

#[test]
fn test_button_role_given_the_delete_button_returns_destructive() {
    // Arrange
    let dialog = dialog("delete_ride_dlg");

    // Act
    let role = button_role(dialog, Button::Delete);

    // Assert
    assert_eq!(role, DialogRole::Destructive);
}

#[test]
fn test_button_role_given_a_destructive_dialogs_cancel_returns_accept() {
    // Arrange: Delete Ride defaults to Cancel, so Enter must accept the safe button.
    let dialog = dialog("delete_ride_dlg");

    // Act
    let role = button_role(dialog, Button::Cancel);

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_a_library_close_returns_cancel() {
    // Arrange: the ride library accepts on Open, so Close leaves without an outcome.
    let dialog = dialog("ride_library_dlg");

    // Act
    let role = button_role(dialog, Button::Close);

    // Assert
    assert_eq!(role, DialogRole::Cancel);
}

#[test]
fn test_button_role_given_the_quit_that_keeps_the_ride_returns_destructive() {
    // Arrange: Quit — keep ride running is neither the default nor the cancel.
    let dialog = dialog("exit_running_dlg");

    // Act
    let role = button_role(dialog, Button::Ok);

    // Assert
    assert_eq!(role, DialogRole::Destructive);
}

#[test]
fn test_button_role_given_the_exit_cancel_returns_accept() {
    // Arrange: Cancel is the safe default of Exit Running.
    let dialog = dialog("exit_running_dlg");

    // Act
    let role = button_role(dialog, Button::Cancel);

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_a_secondary_custom_button_returns_other() {
    // Arrange
    let dialog = dialog("resume_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("library_btn"));

    // Assert
    assert_eq!(role, DialogRole::Other);
}

#[test]
fn test_button_role_given_the_resume_default_returns_accept() {
    // Arrange
    let dialog = dialog("resume_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("continue_btn"));

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_the_simulator_go_button_returns_accept() {
    // Arrange
    let dialog = dialog("simulation_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("go_btn"));

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_the_crossing_delete_returns_destructive() {
    // Arrange
    let dialog = dialog("crossing_detail_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("delete_btn"));

    // Assert
    assert_eq!(role, DialogRole::Destructive);
}

#[test]
fn test_button_role_given_a_single_cancel_running_dialog_returns_accept() {
    // Arrange: the running dialogs' only button is both default and cancel.
    let dialog = dialog("sim_running_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("cancel_btn"));

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_the_crossing_void_card_returns_destructive() {
    // Arrange
    let dialog = dialog("crossing_detail_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("void_card_btn"));

    // Assert
    assert_eq!(role, DialogRole::Destructive);
}

#[test]
fn test_least_width_given_a_table_dialog_returns_the_wide_role() {
    // Arrange
    let dialog = dialog("ride_library_dlg");

    // Act
    let least = least_width(dialog);

    // Assert
    assert_eq!(least, LeastWidth::WideDialog);
}

#[test]
fn test_least_width_given_a_narrow_dialog_returns_the_dialog_role() {
    // Arrange
    let dialog = dialog("about_dlg");

    // Act
    let least = least_width(dialog);

    // Assert
    assert_eq!(least, LeastWidth::Dialog);
}

#[test]
fn test_activation_given_no_close_returns_none() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(None, None, dialog);

    // Assert
    assert_eq!(found, None);
}

#[test]
fn test_activation_given_escape_returns_escape() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(Some(CloseReason::Escape), None, dialog);

    // Assert
    assert_eq!(found, Some(Activation::Escape));
}

#[test]
fn test_activation_given_a_close_action_returns_escape() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(Some(CloseReason::Requested), None, dialog);

    // Assert
    assert_eq!(found, Some(Activation::Escape));
}

#[test]
fn test_activation_given_an_unexpected_close_returns_escape() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(Some(CloseReason::Timeout), None, dialog);

    // Assert
    assert_eq!(found, Some(Activation::Escape));
}

#[test]
fn test_activation_given_enter_returns_the_default_button() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(Some(CloseReason::Enter), None, dialog);

    // Assert
    assert_eq!(found, Some(Activation::Button(Button::Ok)));
}

#[test]
fn test_activation_given_enter_on_a_destructive_dialog_returns_cancel() {
    // Arrange: Enter never runs Delete Ride's destructive action.
    let dialog = dialog("delete_ride_dlg");

    // Act
    let found = activation(Some(CloseReason::Enter), None, dialog);

    // Assert
    assert_eq!(found, Some(Activation::Button(Button::Cancel)));
}

#[test]
fn test_activation_given_a_picked_index_returns_that_button() {
    // Arrange: Delete Ride lists Delete first and Cancel second.
    let dialog = dialog("delete_ride_dlg");

    // Act
    let found = activation(Some(CloseReason::Activated), Some(0), dialog);

    // Assert
    assert_eq!(found, Some(Activation::Button(Button::Delete)));
}

#[test]
fn test_activation_given_a_pick_without_a_close_returns_that_button() {
    // Arrange: a button that stays open still reports its pick.
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(None, Some(1), dialog);

    // Assert
    assert_eq!(found, Some(Activation::Button(Button::Cancel)));
}

#[test]
fn test_activation_given_an_out_of_range_pick_returns_none() {
    // Arrange
    let dialog = dialog("about_dlg");

    // Act
    let found = activation(Some(CloseReason::Activated), Some(9), dialog);

    // Assert
    assert_eq!(found, None);
}
