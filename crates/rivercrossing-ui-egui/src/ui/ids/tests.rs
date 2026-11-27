// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the frozen control registry (`design/docs-md/xrc-windows.md`).

use std::collections::BTreeSet;

use super::{APP_QUIT, FieldKind, FieldSpec, WINDOWS, controls, fields};

#[test]
fn test_the_window_table_has_30_windows() {
    // Act
    let names: Vec<&'static str> = WINDOWS.iter().map(|entry| entry.window).collect();
    let unique: BTreeSet<&'static str> = names.iter().copied().collect();

    // Assert
    assert_eq!(names.len(), 30);
    assert_eq!(unique.len(), names.len());
}

#[test]
fn test_every_window_has_controls() {
    // Act
    let empty: Vec<&'static str> = WINDOWS
        .iter()
        .filter(|entry| entry.controls.is_empty())
        .map(|entry| entry.window)
        .collect();

    // Assert
    assert_eq!(empty, Vec::<&'static str>::new());
}

#[test]
fn test_control_ids_are_unique_within_a_window() {
    // Act
    let repeated: Vec<&'static str> = WINDOWS
        .iter()
        .filter(|entry| {
            let unique: BTreeSet<&'static str> = entry.controls.iter().copied().collect();
            unique.len() != entry.controls.len()
        })
        .map(|entry| entry.window)
        .collect();

    // Assert
    assert_eq!(repeated, Vec::<&'static str>::new());
}

#[test]
fn test_a_few_known_controls_are_present() {
    // Act
    let main = controls("main_frame").map(|ids| {
        (
            ids.contains(&"plate_input"),
            ids.contains(&"record_btn"),
            ids.contains(&"crossings_list"),
        )
    });
    let csv = controls("csv_preview_dlg").map(|ids| ids.contains(&"convert_teams_of_one_chk"));

    // Assert
    assert_eq!(main, Some((true, true, true)));
    assert_eq!(csv, Some(true));
}

#[test]
fn test_the_simulation_dialog_lists_the_new_ride_button() {
    // Act
    let found = controls("simulation_dlg").map(|ids| ids.contains(&"new_ride_btn"));

    // Assert
    assert_eq!(found, Some(true));
}

#[test]
fn test_the_settings_dialog_retires_the_mode_radios() {
    // Act
    let settings = controls("settings_dlg").expect("the frozen window");

    // Assert: the combined Theme picker owns the appearance, so the three radios leave the registry.
    assert_eq!(
        settings.to_vec(),
        vec![
            "sound_chk",
            "show_total_times_chk",
            "show_lap_time_chk",
            "verbose_log_chk",
            "avg_speed_spin",
        ]
    );
}

#[test]
fn test_controls_given_an_unknown_window_returns_none() {
    // Act
    let found = controls("no_such_window");

    // Assert
    assert_eq!(found, None);
}

#[test]
fn test_app_quit_is_the_automation_command_id() {
    // Act
    let name = APP_QUIT;

    // Assert
    assert_eq!(name, "app_quit");
}

/// Return the kind of `id` in `window`, when `fields()` holds it.
fn kind_of(all: &[FieldSpec], window: &str, id: &str) -> Option<FieldKind> {
    all.iter()
        .find(|field| field.window == window && field.id == id)
        .map(|field| field.kind)
}

#[test]
fn test_fields_are_derived_for_every_settable_control() {
    // Act
    let all = fields();

    // Assert
    assert_eq!(all.len(), 67);
}

#[test]
fn test_every_field_id_belongs_to_its_window() {
    // Act
    let stray: Vec<&'static str> = fields()
        .iter()
        .filter(|field| controls(field.window).is_none_or(|list| !list.contains(&field.id)))
        .map(|field| field.id)
        .collect();

    // Assert
    assert_eq!(stray, Vec::<&'static str>::new());
}

#[test]
fn test_fields_map_the_documented_suffixes() {
    // Arrange
    let all = fields();

    // Act
    let text_input = kind_of(&all, "main_frame", "plate_input");
    let text_picker = kind_of(&all, "ride_setup_dlg", "date_picker");
    let choice = kind_of(&all, "ride_setup_dlg", "cap_choice");
    let check = kind_of(&all, "csv_preview_dlg", "convert_teams_of_one_chk");
    let spin = kind_of(&all, "settings_dlg", "avg_speed_spin");
    let button = kind_of(&all, "main_frame", "record_btn");
    let list = kind_of(&all, "main_frame", "crossings_list");

    // Assert
    assert_eq!(text_input, Some(FieldKind::Text));
    assert_eq!(text_picker, Some(FieldKind::Text));
    assert_eq!(choice, Some(FieldKind::Choice));
    assert_eq!(check, Some(FieldKind::Check));
    assert_eq!(spin, Some(FieldKind::Spin));
    assert_eq!(button, None);
    assert_eq!(list, None);
}

#[test]
fn test_field_kinds_have_stable_names() {
    // Act
    let names = [
        FieldKind::Text.as_str(),
        FieldKind::Choice.as_str(),
        FieldKind::Check.as_str(),
        FieldKind::Spin.as_str(),
    ];

    // Assert
    assert_eq!(names, ["text", "choice", "check", "spin"]);
}

#[test]
fn test_every_field_is_unique_within_its_window() {
    // Act
    let all = fields();
    let keys: Vec<(&str, &str)> = all.iter().map(|field| (field.window, field.id)).collect();
    let unique: BTreeSet<(&str, &str)> = keys.iter().copied().collect();

    // Assert
    assert_eq!(unique.len(), keys.len());
}
