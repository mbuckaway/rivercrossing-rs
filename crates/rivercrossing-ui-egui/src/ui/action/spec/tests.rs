// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the §15 action table: its size, its names, its labels and its shortcuts.

use std::collections::BTreeSet;

use super::{ACTIONS, Key, NativeDialog, Target, spec};
use crate::ui::action::id::{
    MI_CLEAR_RIDE, MI_DUPLICATE_RIDE, MI_FINISH_RIDE, MI_REOPEN_RIDE, MI_STOP_RIDE,
    MI_UNDO_CROSSING, WX_ID_EXIT,
};
use crate::ui::action::spec::CommandId;

/// The 50 `mi_*` names `spec.md` §15b lists, in menu order.
const SECTION_15B_MI_IDS: [&str; 50] = [
    // File
    "mi_open_library",
    "mi_duplicate_ride",
    "mi_import_csv",
    "mi_export_csv",
    "mi_backup_now",
    "mi_simulation",
    // Ride
    "mi_new_ride",
    "mi_edit_ride",
    "mi_start_ride",
    "mi_stop_ride",
    "mi_set_start_time",
    "mi_finish_ride",
    "mi_reopen_ride",
    "mi_audit_trail",
    "mi_clear_ride",
    // Riders
    "mi_rider_editor",
    "mi_team_editor",
    "mi_check_rider_issues",
    "mi_mark_dnf",
    // Cards
    "mi_undo_crossing",
    "mi_add_crossing_at",
    "mi_deal_manual",
    // Results
    "mi_standings",
    "mi_export_html",
    "mi_export_pdf",
    "mi_export_poster",
    "mi_export_poster_html",
    "mi_export_results_csv",
    "mi_preview_html_browser",
    "mi_preview_poster_html_browser",
    "mi_preview_pdf_browser",
    "mi_publish_wordpress",
    "mi_show_times",
    "mi_laps_board",
    "mi_time_board",
    "mi_full_field",
    "mi_all_cards",
    "mi_show_dns_riders",
    // View
    "mi_show_total_times",
    "mi_show_lap_time",
    "mi_zoom_90",
    "mi_zoom_100",
    "mi_zoom_110",
    "mi_zoom_120",
    "mi_zoom_130",
    "mi_zoom_140",
    "mi_zoom_150",
    // Help
    "mi_user_guide",
    "mi_shortcuts",
    "mi_selftest",
];

/// The three stock ids `spec.md` §15b lists beside the `mi_*` names.
const SECTION_15B_STOCK_IDS: [&str; 3] = ["wxID_PREFERENCES", "wxID_EXIT", "wxID_ABOUT"];

#[test]
fn test_the_action_table_has_53_unique_ids() {
    // Act
    let unique: BTreeSet<&'static str> = ACTIONS.iter().map(|row| row.id.as_str()).collect();

    // Assert
    assert_eq!(ACTIONS.len(), 53);
    assert_eq!(unique.len(), ACTIONS.len());
}

#[test]
fn test_the_table_matches_the_section_15b_name_list() {
    // Arrange
    let expected_mi: BTreeSet<&'static str> = SECTION_15B_MI_IDS.into_iter().collect();
    let expected_stock: BTreeSet<&'static str> = SECTION_15B_STOCK_IDS.into_iter().collect();

    // Act
    let mi: BTreeSet<&'static str> = ACTIONS
        .iter()
        .map(|row| row.id.as_str())
        .filter(|name| name.starts_with("mi_"))
        .collect();
    let stock: BTreeSet<&'static str> = ACTIONS
        .iter()
        .map(|row| row.id.as_str())
        .filter(|name| !name.starts_with("mi_"))
        .collect();

    // Assert
    assert_eq!(mi, expected_mi);
    assert_eq!(stock, expected_stock);
}

#[test]
fn test_every_action_has_a_non_empty_label() {
    // Act
    let blank: Vec<&'static str> = ACTIONS
        .iter()
        .filter(|row| row.label.trim().is_empty())
        .map(|row| row.id.as_str())
        .collect();

    // Assert
    assert_eq!(blank, Vec::<&'static str>::new());
}

#[test]
fn test_shortcuts_are_only_the_documented_three() {
    // Act
    let bound: Vec<(&'static str, Key, bool)> = ACTIONS
        .iter()
        .filter_map(|row| {
            row.shortcut
                .map(|shortcut| (row.id.as_str(), shortcut.key, shortcut.command))
        })
        .collect();

    // Assert
    assert_eq!(
        bound,
        vec![
            ("mi_undo_crossing", Key::Z, true),
            ("mi_standings", Key::F5, false),
            ("mi_user_guide", Key::F1, false),
        ]
    );
}

#[test]
fn test_every_window_target_is_a_frozen_window() {
    // Act
    let unknown: Vec<&'static str> = ACTIONS
        .iter()
        .filter_map(|row| match row.target {
            Target::Window(window) => Some(window),
            Target::Command(_) | Target::Check(_) | Target::Zoom(_) | Target::Native { .. } => None,
        })
        .filter(|window| crate::ui::ids::controls(window).is_none())
        .collect();

    // Assert
    assert_eq!(unknown, Vec::<&'static str>::new());
}

#[test]
fn test_the_native_rows_name_their_follow_on_command() {
    // Arrange
    let expected = [
        (
            MI_DUPLICATE_RIDE,
            NativeDialog::Prompt,
            CommandId::DuplicateRide,
        ),
        (MI_STOP_RIDE, NativeDialog::Confirm, CommandId::StopRide),
        (MI_FINISH_RIDE, NativeDialog::Danger, CommandId::FinishRide),
        (MI_REOPEN_RIDE, NativeDialog::Prompt, CommandId::ReopenRide),
        (
            MI_UNDO_CROSSING,
            NativeDialog::Danger,
            CommandId::UndoCrossing,
        ),
        (MI_CLEAR_RIDE, NativeDialog::Danger, CommandId::ClearRide),
        (WX_ID_EXIT, NativeDialog::Confirm, CommandId::AppQuit),
    ];

    // Act
    let actual: Vec<Option<Target>> = expected
        .iter()
        .map(|(id, _, _)| spec(*id).map(|row| row.target))
        .collect();
    let wanted: Vec<Option<Target>> = expected
        .iter()
        .map(|(_, dialog, then)| {
            Some(Target::Native {
                dialog: *dialog,
                then: *then,
            })
        })
        .collect();
    let native_rows = ACTIONS
        .iter()
        .filter(|row| matches!(row.target, Target::Native { .. }))
        .count();

    // Assert
    assert_eq!(actual, wanted);
    assert_eq!(native_rows, 7);
}
