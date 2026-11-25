// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the frozen dialog contracts (`design/docs-md/xrc-windows.md`, section B).

use std::collections::BTreeSet;

use rstest::rstest;

use super::{Button, DIALOGS, spec};
use crate::ui::ids;

/// The eight dialogs EPIC-12 Phase 3 declares, in table order.
const PHASE_3_IDS: [&str; 8] = [
    "ride_setup_dlg",
    "start_blocked_dlg",
    "exit_running_dlg",
    "delete_ride_dlg",
    "resume_dlg",
    "set_start_dlg",
    "about_dlg",
    "shortcuts_dlg",
];

/// Return true when the window declares the control id.
fn window_has_control(window: &str, control: &str) -> bool {
    ids::controls(window).is_some_and(|controls| controls.contains(&control))
}

/// Return true when the window declares the field as a settable control.
fn window_has_field(window: &str, field: &str) -> bool {
    ids::fields()
        .iter()
        .any(|entry| entry.window == window && entry.id == field)
}

/// Return the frozen control id of a custom button, or `None` for a stock button.
fn custom_id(button: Button) -> Option<&'static str> {
    match button {
        Button::Custom(id) => Some(id),
        Button::Ok | Button::Cancel | Button::Close | Button::Delete => None,
    }
}

#[test]
fn test_the_table_holds_the_eight_phase_3_dialogs() {
    // Act
    let ids: Vec<&'static str> = DIALOGS.iter().map(|dialog| dialog.id).collect();

    // Assert
    assert_eq!(ids, PHASE_3_IDS.to_vec());
}

#[test]
fn test_dialog_ids_are_unique() {
    // Act
    let ids: Vec<&'static str> = DIALOGS.iter().map(|dialog| dialog.id).collect();
    let unique: BTreeSet<&'static str> = ids.iter().copied().collect();

    // Assert
    assert_eq!(unique.len(), ids.len());
}

#[test]
fn test_dialog_ids_are_frozen_window_ids() {
    // Act
    let unknown: Vec<&'static str> = DIALOGS
        .iter()
        .filter(|dialog| ids::controls(dialog.id).is_none())
        .map(|dialog| dialog.id)
        .collect();

    // Assert
    assert_eq!(unknown, Vec::<&'static str>::new());
}

#[test]
fn test_every_custom_button_is_a_frozen_control() {
    // Act
    let stray: Vec<&'static str> = DIALOGS
        .iter()
        .flat_map(|dialog| {
            dialog
                .buttons
                .iter()
                .map(move |button| (dialog.id, *button))
        })
        .filter_map(|(id, button)| {
            custom_id(button).filter(|control| !window_has_control(id, control))
        })
        .collect();

    // Assert
    assert_eq!(stray, Vec::<&'static str>::new());
}

#[test]
fn test_every_first_field_is_a_settable_control() {
    // Act
    let stray: Vec<&'static str> = DIALOGS
        .iter()
        .filter_map(|dialog| {
            dialog
                .first_field
                .filter(|field| !window_has_field(dialog.id, field))
        })
        .collect();

    // Assert
    assert_eq!(stray, Vec::<&'static str>::new());
}

#[test]
fn test_every_spec_lists_each_button_once() {
    // Act
    let repeated: Vec<&'static str> = DIALOGS
        .iter()
        .filter(|dialog| {
            let unique: BTreeSet<&'static str> = dialog
                .buttons
                .iter()
                .map(|button| button.as_str())
                .collect();
            unique.len() != dialog.buttons.len()
        })
        .map(|dialog| dialog.id)
        .collect();

    // Assert
    assert_eq!(repeated, Vec::<&'static str>::new());
}

#[test]
fn test_every_spec_has_the_same_default_and_cancel_family() {
    // Act
    let bad_cancel: Vec<&'static str> = DIALOGS
        .iter()
        .filter(|dialog| !matches!(dialog.cancel, Button::Cancel | Button::Close))
        .map(|dialog| dialog.id)
        .collect();
    let enter_delete: Vec<&'static str> = DIALOGS
        .iter()
        .filter(|dialog| dialog.default == Button::Delete)
        .map(|dialog| dialog.id)
        .collect();
    let destructive_not_cancel: Vec<&'static str> = DIALOGS
        .iter()
        .filter(|dialog| dialog.destructive && dialog.default != dialog.cancel)
        .map(|dialog| dialog.id)
        .collect();
    let unrendered_default: Vec<&'static str> = DIALOGS
        .iter()
        .filter(|dialog| !dialog.buttons.contains(&dialog.default))
        .map(|dialog| dialog.id)
        .collect();

    // Assert
    assert_eq!(bad_cancel, Vec::<&'static str>::new());
    assert_eq!(enter_delete, Vec::<&'static str>::new());
    assert_eq!(destructive_not_cancel, Vec::<&'static str>::new());
    assert_eq!(unrendered_default, Vec::<&'static str>::new());
}

#[rstest]
#[case("ride_setup_dlg", "Ride Setup")]
#[case("start_blocked_dlg", "Cannot Start Ride")]
#[case("exit_running_dlg", "Ride Is Running")]
#[case("delete_ride_dlg", "Delete Ride")]
#[case("resume_dlg", "Resume Ride")]
#[case("set_start_dlg", "Set Start Time")]
#[case("about_dlg", "About RiverCrossing")]
#[case("shortcuts_dlg", "Keyboard Shortcuts")]
fn test_every_dialog_has_the_documented_title(#[case] id: &str, #[case] title: &str) {
    // Act
    let found = spec(id).map(|dialog| dialog.title);

    // Assert
    assert_eq!(found, Some(title));
}

#[test]
fn test_spec_returns_the_dialog_row() {
    // Act
    let found = spec("delete_ride_dlg").map(|dialog| {
        (
            dialog.default,
            dialog.cancel,
            dialog.destructive,
            dialog.first_field,
        )
    });

    // Assert
    assert_eq!(found, Some((Button::Cancel, Button::Cancel, true, None)));
}

#[test]
fn test_spec_given_an_unknown_id_returns_none() {
    // Act
    let found = spec("no_such_dlg");

    // Assert
    assert_eq!(found, None);
}

#[rstest]
#[case(Button::Ok, "OK")]
#[case(Button::Cancel, "Cancel")]
#[case(Button::Close, "Close")]
#[case(Button::Delete, "Delete")]
fn test_button_as_str_returns_the_stock_label(#[case] button: Button, #[case] label: &str) {
    // Act
    let rendered = button.as_str();

    // Assert
    assert_eq!(rendered, label);
}

#[test]
fn test_button_as_str_returns_the_frozen_custom_id() {
    // Act
    let rendered = Button::Custom("finish_first_btn").as_str();

    // Assert
    assert_eq!(rendered, "finish_first_btn");
}
