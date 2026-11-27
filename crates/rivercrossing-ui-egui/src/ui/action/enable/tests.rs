// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the §15 "Enabled when" matrix.

use rstest::rstest;

use super::{GateState, RidePhase, enabled};
use crate::ui::action::id::{
    ActionId, MI_AUDIT_TRAIL, MI_CLEAR_RIDE, MI_EXPORT_HTML, MI_LAPS_BOARD, MI_NEW_RIDE,
    MI_PREVIEW_HTML_BROWSER, MI_PREVIEW_PDF_BROWSER, MI_PREVIEW_POSTER_HTML_BROWSER,
    MI_REOPEN_RIDE, MI_SHOW_TIMES, MI_SIMULATION, MI_START_RIDE, MI_STOP_RIDE, MI_TEAM_EDITOR,
    MI_TIME_BOARD, MI_UNDO_CROSSING,
};
use crate::ui::action::spec::ACTIONS;

/// A gate in `phase` with every optional input off.
fn gate(phase: RidePhase) -> GateState {
    GateState {
        phase,
        ..GateState::default()
    }
}

/// A gate in `phase` with every optional input on.
fn open_gate(phase: RidePhase) -> GateState {
    GateState {
        phase,
        teams_enabled: true,
        has_crossing: true,
        has_audit_row: true,
        html_export: true,
        poster_export: true,
        pdf_export: true,
        show_times: true,
    }
}

#[rstest]
#[case(
    RidePhase::None,
    &[
        "mi_open_library",
        "mi_backup_now",
        "mi_simulation",
        "wxID_PREFERENCES",
        "wxID_EXIT",
        "mi_new_ride",
        "mi_standings",
        "mi_show_times",
        "mi_laps_board",
        "mi_time_board",
        "mi_full_field",
        "mi_all_cards",
        "mi_show_dns_riders",
        "mi_show_total_times",
        "mi_show_lap_time",
        "mi_density_compact",
        "mi_density_comfortable",
        "mi_text_size_tiny",
        "mi_text_size_small",
        "mi_text_size_medium",
        "mi_text_size_large",
        "mi_user_guide",
        "mi_shortcuts",
        "mi_selftest",
        "wxID_ABOUT",
    ]
)]
#[case(
    RidePhase::Draft,
    &[
        "mi_open_library",
        "mi_duplicate_ride",
        "mi_import_csv",
        "mi_export_csv",
        "mi_backup_now",
        "mi_simulation",
        "wxID_PREFERENCES",
        "wxID_EXIT",
        "mi_edit_ride",
        "mi_start_ride",
        "mi_audit_trail",
        "mi_clear_ride",
        "mi_rider_editor",
        "mi_team_editor",
        "mi_check_rider_issues",
        "mi_standings",
        "mi_show_times",
        "mi_laps_board",
        "mi_time_board",
        "mi_full_field",
        "mi_all_cards",
        "mi_show_dns_riders",
        "mi_show_total_times",
        "mi_show_lap_time",
        "mi_density_compact",
        "mi_density_comfortable",
        "mi_text_size_tiny",
        "mi_text_size_small",
        "mi_text_size_medium",
        "mi_text_size_large",
        "mi_user_guide",
        "mi_shortcuts",
        "mi_selftest",
        "wxID_ABOUT",
    ]
)]
#[case(
    RidePhase::Running,
    &[
        "mi_open_library",
        "mi_duplicate_ride",
        "mi_import_csv",
        "mi_export_csv",
        "mi_backup_now",
        "wxID_PREFERENCES",
        "wxID_EXIT",
        "mi_edit_ride",
        "mi_stop_ride",
        "mi_set_start_time",
        "mi_finish_ride",
        "mi_audit_trail",
        "mi_rider_editor",
        "mi_team_editor",
        "mi_check_rider_issues",
        "mi_mark_dnf",
        "mi_undo_crossing",
        "mi_add_crossing_at",
        "mi_deal_manual",
        "mi_standings",
        "mi_show_times",
        "mi_laps_board",
        "mi_time_board",
        "mi_full_field",
        "mi_all_cards",
        "mi_show_dns_riders",
        "mi_show_total_times",
        "mi_show_lap_time",
        "mi_density_compact",
        "mi_density_comfortable",
        "mi_text_size_tiny",
        "mi_text_size_small",
        "mi_text_size_medium",
        "mi_text_size_large",
        "mi_user_guide",
        "mi_shortcuts",
        "mi_selftest",
        "wxID_ABOUT",
    ]
)]
#[case(
    RidePhase::RunningStopped,
    &[
        "mi_open_library",
        "mi_duplicate_ride",
        "mi_import_csv",
        "mi_export_csv",
        "mi_backup_now",
        "mi_simulation",
        "wxID_PREFERENCES",
        "wxID_EXIT",
        "mi_edit_ride",
        "mi_start_ride",
        "mi_audit_trail",
        "mi_clear_ride",
        "mi_rider_editor",
        "mi_team_editor",
        "mi_check_rider_issues",
        "mi_standings",
        "mi_show_times",
        "mi_laps_board",
        "mi_time_board",
        "mi_full_field",
        "mi_all_cards",
        "mi_show_dns_riders",
        "mi_show_total_times",
        "mi_show_lap_time",
        "mi_density_compact",
        "mi_density_comfortable",
        "mi_text_size_tiny",
        "mi_text_size_small",
        "mi_text_size_medium",
        "mi_text_size_large",
        "mi_user_guide",
        "mi_shortcuts",
        "mi_selftest",
        "wxID_ABOUT",
    ]
)]
#[case(
    RidePhase::Finished,
    &[
        "mi_open_library",
        "mi_duplicate_ride",
        "mi_import_csv",
        "mi_export_csv",
        "mi_backup_now",
        "wxID_PREFERENCES",
        "wxID_EXIT",
        "mi_edit_ride",
        "mi_reopen_ride",
        "mi_audit_trail",
        "mi_clear_ride",
        "mi_rider_editor",
        "mi_team_editor",
        "mi_check_rider_issues",
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
        "mi_show_total_times",
        "mi_show_lap_time",
        "mi_density_compact",
        "mi_density_comfortable",
        "mi_text_size_tiny",
        "mi_text_size_small",
        "mi_text_size_medium",
        "mi_text_size_large",
        "mi_user_guide",
        "mi_shortcuts",
        "mi_selftest",
        "wxID_ABOUT",
    ]
)]
#[case(
    RidePhase::Reopened,
    &[
        "mi_open_library",
        "mi_duplicate_ride",
        "mi_import_csv",
        "mi_export_csv",
        "mi_backup_now",
        "wxID_PREFERENCES",
        "wxID_EXIT",
        "mi_edit_ride",
        "mi_start_ride",
        "mi_set_start_time",
        "mi_finish_ride",
        "mi_audit_trail",
        "mi_clear_ride",
        "mi_rider_editor",
        "mi_team_editor",
        "mi_check_rider_issues",
        "mi_mark_dnf",
        "mi_add_crossing_at",
        "mi_deal_manual",
        "mi_standings",
        "mi_show_times",
        "mi_laps_board",
        "mi_time_board",
        "mi_full_field",
        "mi_all_cards",
        "mi_show_dns_riders",
        "mi_show_total_times",
        "mi_show_lap_time",
        "mi_density_compact",
        "mi_density_comfortable",
        "mi_text_size_tiny",
        "mi_text_size_small",
        "mi_text_size_medium",
        "mi_text_size_large",
        "mi_user_guide",
        "mi_shortcuts",
        "mi_selftest",
        "wxID_ABOUT",
    ]
)]
fn test_enabled_matrix_per_ride_phase(#[case] phase: RidePhase, #[case] expected: &[&str]) {
    // Arrange
    let state = open_gate(phase);

    // Act
    let actual: Vec<&'static str> = ACTIONS
        .iter()
        .filter(|row| enabled(row.id, &state))
        .map(|row| row.id.as_str())
        .collect();

    // Assert
    assert_eq!(actual, expected);
}

#[test]
fn test_new_ride_only_with_no_ride_open() {
    // Arrange
    let no_ride = gate(RidePhase::None);
    let draft = gate(RidePhase::Draft);

    // Act
    let with_no_ride = enabled(MI_NEW_RIDE, &no_ride);
    let with_a_ride = enabled(MI_NEW_RIDE, &draft);

    // Assert
    assert!(with_no_ride);
    assert!(!with_a_ride);
}

#[test]
fn test_start_enabled_in_draft_reopened_and_stopped_running() {
    // Act
    let draft = enabled(MI_START_RIDE, &gate(RidePhase::Draft));
    let reopened = enabled(MI_START_RIDE, &gate(RidePhase::Reopened));
    let stopped = enabled(MI_START_RIDE, &gate(RidePhase::RunningStopped));
    let running = enabled(MI_START_RIDE, &gate(RidePhase::Running));
    let finished = enabled(MI_START_RIDE, &gate(RidePhase::Finished));
    let none = enabled(MI_START_RIDE, &gate(RidePhase::None));

    // Assert
    assert!(draft);
    assert!(reopened);
    assert!(stopped);
    assert!(!running);
    assert!(!finished);
    assert!(!none);
}

#[test]
fn test_stop_needs_a_live_running_ride() {
    // Act
    let running = enabled(MI_STOP_RIDE, &gate(RidePhase::Running));
    let stopped = enabled(MI_STOP_RIDE, &gate(RidePhase::RunningStopped));
    let reopened = enabled(MI_STOP_RIDE, &gate(RidePhase::Reopened));

    // Assert
    assert!(running);
    assert!(!stopped);
    assert!(!reopened);
}

#[test]
fn test_simulation_needs_draft_or_stopped_running() {
    // Act
    let none = enabled(MI_SIMULATION, &gate(RidePhase::None));
    let draft = enabled(MI_SIMULATION, &gate(RidePhase::Draft));
    let stopped = enabled(MI_SIMULATION, &gate(RidePhase::RunningStopped));
    let running = enabled(MI_SIMULATION, &gate(RidePhase::Running));
    let finished = enabled(MI_SIMULATION, &gate(RidePhase::Finished));

    // Assert
    assert!(none);
    assert!(draft);
    assert!(stopped);
    assert!(!running);
    assert!(!finished);
}

#[test]
fn test_undo_needs_a_crossing() {
    // Arrange
    let running = gate(RidePhase::Running);
    let running_with_a_crossing = GateState {
        has_crossing: true,
        ..running
    };
    let reopened_with_a_crossing = GateState {
        phase: RidePhase::Reopened,
        has_crossing: true,
        ..GateState::default()
    };

    // Act
    let without = enabled(MI_UNDO_CROSSING, &running);
    let with = enabled(MI_UNDO_CROSSING, &running_with_a_crossing);
    let reopened = enabled(MI_UNDO_CROSSING, &reopened_with_a_crossing);

    // Assert
    assert!(!without);
    assert!(with);
    assert!(!reopened);
}

#[test]
fn test_exports_and_previews_need_finished_and_their_own_format() {
    // Arrange
    let draft = open_gate(RidePhase::Draft);
    let finished = gate(RidePhase::Finished);
    let html_only = GateState {
        html_export: true,
        ..gate(RidePhase::Finished)
    };
    let poster_only = GateState {
        poster_export: true,
        ..gate(RidePhase::Finished)
    };
    let pdf_only = GateState {
        pdf_export: true,
        ..gate(RidePhase::Finished)
    };

    // Act and assert
    assert!(!enabled(MI_EXPORT_HTML, &draft));
    assert!(enabled(MI_EXPORT_HTML, &finished));
    assert!(!enabled(MI_PREVIEW_HTML_BROWSER, &draft));
    assert!(!enabled(MI_PREVIEW_POSTER_HTML_BROWSER, &draft));
    assert!(!enabled(MI_PREVIEW_PDF_BROWSER, &draft));
    assert!(!enabled(MI_PREVIEW_HTML_BROWSER, &finished));
    assert!(enabled(MI_PREVIEW_HTML_BROWSER, &html_only));
    assert!(!enabled(MI_PREVIEW_POSTER_HTML_BROWSER, &html_only));
    assert!(enabled(MI_PREVIEW_POSTER_HTML_BROWSER, &poster_only));
    assert!(!enabled(MI_PREVIEW_PDF_BROWSER, &poster_only));
    assert!(enabled(MI_PREVIEW_PDF_BROWSER, &pdf_only));
}

#[test]
fn test_time_board_needs_show_times() {
    // Arrange
    let off = gate(RidePhase::None);
    let on = GateState {
        show_times: true,
        ..GateState::default()
    };

    // Act
    let hidden = enabled(MI_TIME_BOARD, &off);
    let shown = enabled(MI_TIME_BOARD, &on);
    let enabler = enabled(MI_SHOW_TIMES, &off);

    // Assert
    assert!(!hidden);
    assert!(shown);
    assert!(enabler);
}

#[test]
fn test_team_editor_needs_teams() {
    // Arrange
    let mixed = GateState {
        phase: RidePhase::Draft,
        teams_enabled: true,
        ..GateState::default()
    };
    let solo = gate(RidePhase::Draft);
    let no_ride = GateState {
        teams_enabled: true,
        ..GateState::default()
    };

    // Act
    let mixed_ride = enabled(MI_TEAM_EDITOR, &mixed);
    let solo_ride = enabled(MI_TEAM_EDITOR, &solo);
    let closed = enabled(MI_TEAM_EDITOR, &no_ride);

    // Assert
    assert!(mixed_ride);
    assert!(!solo_ride);
    assert!(!closed);
}

#[test]
fn test_audit_needs_a_row() {
    // Arrange
    let with_row = GateState {
        phase: RidePhase::Draft,
        has_audit_row: true,
        ..GateState::default()
    };
    let without_row = gate(RidePhase::Draft);
    let no_ride = GateState {
        has_audit_row: true,
        ..GateState::default()
    };

    // Act
    let listed = enabled(MI_AUDIT_TRAIL, &with_row);
    let empty = enabled(MI_AUDIT_TRAIL, &without_row);
    let closed = enabled(MI_AUDIT_TRAIL, &no_ride);

    // Assert
    assert!(listed);
    assert!(!empty);
    assert!(!closed);
}

#[test]
fn test_clear_needs_a_loaded_ride_in_a_clearable_phase() {
    // Act
    let draft = enabled(MI_CLEAR_RIDE, &gate(RidePhase::Draft));
    let running = enabled(MI_CLEAR_RIDE, &gate(RidePhase::Running));
    let stopped = enabled(MI_CLEAR_RIDE, &gate(RidePhase::RunningStopped));
    let finished = enabled(MI_CLEAR_RIDE, &gate(RidePhase::Finished));
    let reopened = enabled(MI_CLEAR_RIDE, &gate(RidePhase::Reopened));
    let none = enabled(MI_CLEAR_RIDE, &gate(RidePhase::None));

    // Assert
    assert!(draft);
    assert!(!running);
    assert!(stopped);
    assert!(finished);
    assert!(reopened);
    assert!(!none);
}

#[test]
fn test_reopen_needs_a_finished_ride() {
    // Act
    let finished = enabled(MI_REOPEN_RIDE, &gate(RidePhase::Finished));
    let reopened = enabled(MI_REOPEN_RIDE, &gate(RidePhase::Reopened));

    // Assert
    assert!(finished);
    assert!(!reopened);
}

#[test]
fn test_laps_board_is_always_available() {
    // Act
    let closed = enabled(MI_LAPS_BOARD, &gate(RidePhase::None));
    let running = enabled(MI_LAPS_BOARD, &gate(RidePhase::Running));

    // Assert
    assert!(closed);
    assert!(running);
}

#[test]
fn test_enabled_given_an_unknown_id_is_false() {
    // Arrange
    let unknown = ActionId::from_name("not_an_action");

    // Act
    let enabled_now = enabled(unknown, &open_gate(RidePhase::Running));

    // Assert
    assert!(!enabled_now);
}
