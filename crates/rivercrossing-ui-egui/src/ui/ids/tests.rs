// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the frozen control registry (`design/docs-md/xrc-windows.md`).

use std::collections::BTreeSet;

use super::{APP_QUIT, WINDOWS, controls};

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
