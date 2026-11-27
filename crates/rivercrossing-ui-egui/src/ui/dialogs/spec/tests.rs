// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the frozen dialog contracts (`design/docs-md/xrc-windows.md`, section B).

use std::collections::BTreeSet;

use rstest::rstest;

use super::{Button, DIALOGS, spec};
use crate::ui::ids;

/// The dialogs the registry declares, in table order.
const DIALOG_IDS: [&str; 29] = [
    "ride_setup_dlg",
    "start_blocked_dlg",
    "exit_running_dlg",
    "delete_ride_dlg",
    "resume_dlg",
    "set_start_dlg",
    "about_dlg",
    "shortcuts_dlg",
    "rider_editor_dlg",
    "add_rider_dlg",
    "csv_preview_dlg",
    "team_editor_dlg",
    "add_team_dlg",
    "rider_issues_dlg",
    "edit_crossing_dlg",
    "manual_deal_dlg",
    "dnf_confirm_dlg",
    "void_card_confirm_dlg",
    "ride_library_dlg",
    "audit_dlg",
    "settings_dlg",
    "selftest_dlg",
    "crossing_detail_dlg",
    "crossing_number_dlg",
    "results_dlg",
    "publish_wordpress_dlg",
    "publish_running_dlg",
    "simulation_dlg",
    "sim_running_dlg",
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
fn test_the_table_holds_the_twenty_nine_dialogs() {
    // Act
    let ids: Vec<&'static str> = DIALOGS.iter().map(|dialog| dialog.id).collect();

    // Assert
    assert_eq!(ids, DIALOG_IDS.to_vec());
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

/// Return the window's listed controls that end in the button suffix.
fn listed_buttons(window: &str) -> Vec<&'static str> {
    ids::controls(window).map_or_else(Vec::new, |controls| {
        controls
            .iter()
            .copied()
            .filter(|control| control.ends_with("_btn"))
            .collect()
    })
}

/// Return the position of *button* in the dialog's visual order.
fn position(dialog: &super::DialogSpec, button: Button) -> Option<usize> {
    dialog.buttons.iter().position(|found| *found == button)
}

/// A window that declares any custom button draws its whole button row from the spec, so every
/// frozen `_btn` it lists must be declared: the dialog host renders only the declared buttons, and
/// a listed-but-undeclared id (the `go_btn` hole) would be silent. A window that declares no custom
/// button draws its extra buttons from its own body view, so its registry buttons are not the
/// spec's to declare.
#[test]
fn test_a_custom_button_window_declares_every_listed_button() {
    // Act
    let undeclared: Vec<(&'static str, &'static str)> = DIALOGS
        .iter()
        .filter(|dialog| {
            dialog
                .buttons
                .iter()
                .any(|button| custom_id(*button).is_some())
        })
        .flat_map(|dialog| {
            listed_buttons(dialog.id)
                .into_iter()
                .filter(|control| position(dialog, Button::Custom(control)).is_none())
                .map(move |control| (dialog.id, control))
        })
        .collect();

    // Assert
    assert_eq!(undeclared, Vec::<(&'static str, &'static str)>::new());
}

#[test]
fn test_the_simulator_spec_places_new_ride_left_of_generate_riders() {
    // Act
    let order = spec("simulation_dlg").map(|dialog| {
        (
            position(dialog, Button::Custom("new_ride_btn")),
            position(dialog, Button::Custom("gen_riders_btn")),
        )
    });

    // Assert
    assert_eq!(order, Some((Some(0), Some(1))));
}

#[test]
fn test_the_simulator_default_is_the_go_button() {
    // Act
    let found = spec("simulation_dlg")
        .map(|dialog| (dialog.default, position(dialog, Button::Custom("go_btn"))));

    // Assert
    assert_eq!(found, Some((Button::Custom("go_btn"), Some(3))));
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
        .filter(|dialog| {
            !matches!(
                dialog.cancel,
                Button::Cancel | Button::Close | Button::Custom("cancel_btn")
            )
        })
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
#[case("rider_editor_dlg", "Rider Editor")]
#[case("add_rider_dlg", "Add Rider")]
#[case("csv_preview_dlg", "Import Riders")]
#[case("team_editor_dlg", "Teams Editor")]
#[case("add_team_dlg", "Add Team")]
#[case("rider_issues_dlg", "Rider Issues")]
#[case("edit_crossing_dlg", "Edit Crossing")]
#[case("manual_deal_dlg", "Deal Bonus Card")]
#[case("dnf_confirm_dlg", "Mark DNF")]
#[case("void_card_confirm_dlg", "Void Card")]
#[case("ride_library_dlg", "Ride Library")]
#[case("audit_dlg", "Audit Trail")]
#[case("settings_dlg", "Settings")]
#[case("selftest_dlg", "Evaluator Self-Test")]
#[case("crossing_detail_dlg", "Crossing Detail")]
#[case("crossing_number_dlg", "Plate")]
#[case("results_dlg", "Standings")]
#[case("publish_wordpress_dlg", "Publish to WordPress")]
#[case("publish_running_dlg", "Publishing to WordPress")]
#[case("simulation_dlg", "Rider Simulator")]
#[case("sim_running_dlg", "Simulating…")]
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
    assert_eq!(
        found,
        Some((
            Button::Cancel,
            Button::Cancel,
            true,
            Some("confirm_name_input")
        ))
    );
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

#[test]
fn test_every_window_besides_main_frame_has_a_dialog_spec() {
    // Act
    let missing: Vec<&'static str> = ids::WINDOWS
        .iter()
        .map(|window| window.window)
        .filter(|window| *window != "main_frame")
        .filter(|window| spec(window).is_none())
        .collect();

    // Assert
    assert!(
        missing.is_empty(),
        "windows without a DialogSpec: {missing:?}"
    );
}
