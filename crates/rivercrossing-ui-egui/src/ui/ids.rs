// SPDX-License-Identifier: GPL-3.0-only

//! The frozen §15b control registry: one entry per top-level window.
//!
//! Every control name is the `xrc_name` annotation in `design/docs-md/xrc-windows.md`, in document
//! order. Code-side names (the info bars and the console gauges) are included as the document writes
//! them. Stock dialog ids (`wxID_OK`, `wxID_CANCEL`, …) are ids, not frozen names, so they are not
//! listed.

/// The frozen control ids of one top-level window.
#[derive(Debug)]
pub(crate) struct WindowControls {
    /// The frozen window id.
    pub(crate) window: &'static str,
    /// The frozen control ids of the window, in document order.
    pub(crate) controls: &'static [&'static str],
}

/// The automation-only command id that quits the app (replaces the removed `probe_quit`).
pub(crate) const APP_QUIT: &str = "app_quit";

/// The 30 top-level windows and their frozen control ids.
pub(crate) const WINDOWS: &[WindowControls] = &[
    WindowControls {
        window: "main_frame",
        controls: &[
            "main_menubar",
            "ride_status_panel",
            "ride_status_light",
            "ride_status_lbl",
            "ride_name_value",
            "ride_date_value",
            "ride_venue_value",
            "ride_organizer_value",
            "ride_scorer_value",
            "ride_lap_km_value",
            "ride_logo_bmp",
            "current_lap_lbl",
            "elapsed_clock_panel",
            "remaining_clock_panel",
            "elapsed_clock",
            "remaining_clock",
            "clock_elapsed_lbl",
            "clock_remaining_lbl",
            "start_btn",
            "stop_btn",
            "plate_input",
            "record_btn",
            "last_crossing_lbl",
            "undo_btn",
            "crossings_search",
            "main_splitter",
            "crossings_list",
            "crossings_count_lbl",
            "cards_count_lbl",
            "on_course_lbl",
            "shoe_lbl",
            "riders_count_lbl",
            "teams_count_lbl",
            "review_notebook",
            "console_riders_list",
            "flagged_list",
            "show_held_only_chk",
            "review_btn",
            "main_statusbar",
        ],
    },
    WindowControls {
        window: "ride_setup_dlg",
        controls: &[
            "name_input",
            "date_picker",
            "start_time_picker",
            "venue_input",
            "lap_km_spin",
            "organizer_input",
            "scorer_input",
            "duration_input",
            "min_lap_input",
            "hold_short_radio",
            "always_deal_radio",
            "tiebreak_list",
            "logo_preview_bmp",
            "logo_status_lbl",
            "logo_browse_btn",
            "solo_radio",
            "mixed_radio",
            "team_size_spin",
            "pooled_radio",
            "relay_radio",
            "decks_spin",
            "jokers_spin",
            "jokers_per_deck_radio",
            "jokers_total_radio",
            "cap_choice",
            "setup_infobar",
        ],
    },
    WindowControls {
        window: "set_start_dlg",
        controls: &["start_date_picker", "start_time_picker"],
    },
    WindowControls {
        window: "resume_dlg",
        controls: &["message_lbl", "continue_btn", "library_btn"],
    },
    WindowControls {
        window: "exit_running_dlg",
        controls: &["finish_first_btn", "message_lbl"],
    },
    WindowControls {
        window: "rider_editor_dlg",
        controls: &[
            "rider_search",
            "riders_list",
            "add_btn",
            "delete_btn",
            "plate_input",
            "first_name_input",
            "last_name_input",
            "team_choice",
            "edit_btn",
            "roster_infobar",
        ],
    },
    WindowControls {
        window: "add_rider_dlg",
        controls: &[
            "plate_input",
            "first_name_input",
            "last_name_input",
            "team_choice",
            "sex_choice",
            "add_rider_infobar",
        ],
    },
    WindowControls {
        window: "csv_preview_dlg",
        controls: &[
            "summary_lbl",
            "conflicts_list",
            "map_unknown_sex_chk",
            "convert_teams_of_one_chk",
            "csv_infobar",
        ],
    },
    WindowControls {
        window: "team_editor_dlg",
        controls: &[
            "teams_list",
            "single_member_only_chk",
            "edit_btn",
            "remove_btn",
            "name_input",
            "relay_plate_input",
            "notes_input",
            "members_list",
            "add_btn",
            "teams_infobar",
        ],
    },
    WindowControls {
        window: "add_team_dlg",
        controls: &[
            "name_input",
            "relay_plate_input",
            "notes_input",
            "logo_bmp",
            "pick_card_btn",
            "add_team_infobar",
        ],
    },
    WindowControls {
        window: "rider_issues_dlg",
        controls: &[
            "card_check_lbl",
            "issues_summary_lbl",
            "issues_list",
            "open_editor_btn",
            "convert_solo_btn",
            "assign_plate_btn",
            "renumber_btn",
            "issues_infobar",
        ],
    },
    WindowControls {
        window: "start_blocked_dlg",
        controls: &["start_blocked_list"],
    },
    WindowControls {
        window: "edit_crossing_dlg",
        controls: &["plate_input", "time_picker", "reason_input", "void_btn"],
    },
    WindowControls {
        window: "manual_deal_dlg",
        controls: &["plate_input", "reason_input"],
    },
    WindowControls {
        window: "dnf_confirm_dlg",
        controls: &["plate_input", "entry_lbl", "reason_input"],
    },
    WindowControls {
        window: "void_card_confirm_dlg",
        controls: &["card_lbl", "entry_lbl", "reason_input"],
    },
    WindowControls {
        window: "results_dlg",
        controls: &[
            "results_notebook",
            "teams_standings_list",
            "solo_standings_list",
            "standings_list",
            "stale_infobar",
        ],
    },
    WindowControls {
        window: "ride_library_dlg",
        controls: &["rides_list", "duplicate_btn"],
    },
    WindowControls {
        window: "delete_ride_dlg",
        controls: &["message_lbl", "confirm_name_input"],
    },
    WindowControls {
        window: "audit_dlg",
        controls: &["audit_search", "action_choice", "audit_list"],
    },
    WindowControls {
        window: "settings_dlg",
        controls: &[
            "sound_chk",
            "show_total_times_chk",
            "show_lap_time_chk",
            "verbose_log_chk",
            "avg_speed_spin",
        ],
    },
    WindowControls {
        window: "about_dlg",
        controls: &[
            "version_lbl",
            "author_lbl",
            "copyright_lbl",
            "license_lbl",
            "gorba_link",
        ],
    },
    WindowControls {
        window: "shortcuts_dlg",
        controls: &["shortcuts_list"],
    },
    WindowControls {
        window: "selftest_dlg",
        controls: &["selftest_output", "rerun_btn"],
    },
    WindowControls {
        window: "crossing_detail_dlg",
        controls: &[
            "crossing_rider_lbl",
            "crossing_team_lbl",
            "crossing_plate_lbl",
            "crossing_lap_lbl",
            "crossing_time_lbl",
            "crossing_lap_time_lbl",
            "crossing_total_lbl",
            "crossing_card_lbl",
            "crossing_held_lbl",
            "edit_btn",
            "edit_time_btn",
            "void_card_btn",
            "delete_btn",
            "crossing_detail_infobar",
        ],
    },
    WindowControls {
        window: "crossing_number_dlg",
        controls: &["number_input"],
    },
    WindowControls {
        window: "simulation_dlg",
        controls: &[
            "riders_spin",
            "teams_spin",
            "solo_spin",
            "laps_spin",
            "interval_spin",
            "short_lap_choice",
            "lapped_choice",
            "team_stop_choice",
            "new_ride_btn",
            "gen_riders_btn",
            "check_btn",
            "go_btn",
            "sim_infobar",
        ],
    },
    WindowControls {
        window: "sim_running_dlg",
        controls: &["sim_status_lbl", "progress_gauge", "cancel_btn"],
    },
    WindowControls {
        window: "publish_wordpress_dlg",
        controls: &[
            "wp_kind_choice",
            "wp_url_input",
            "wp_username_input",
            "wp_password_input",
            "wp_title_input",
            "wp_slug_input",
            "wp_parent_input",
            "wp_status_choice",
        ],
    },
    WindowControls {
        window: "publish_running_dlg",
        controls: &["publish_status_lbl", "progress_gauge", "cancel_btn"],
    },
];

/// Return the frozen control ids of the window, or `None` when the window is unknown.
#[must_use]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the drift test reads the frozen controls through this lookup"
    )
)]
pub(crate) fn controls(window: &str) -> Option<&'static [&'static str]> {
    WINDOWS
        .iter()
        .find(|entry| entry.window == window)
        .map(|entry| entry.controls)
}

/// The widget kind of a settable control, as the machine block spells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FieldKind {
    /// A text input or a date/time picker.
    Text,
    /// A dropdown choice.
    Choice,
    /// A checkbox.
    Check,
    /// A numeric spinner.
    Spin,
}

impl FieldKind {
    /// Return the stable name, as the machine block spells it.
    #[must_use]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the machine-block spelling the drift test renders"
        )
    )]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Choice => "choice",
            Self::Check => "check",
            Self::Spin => "spin",
        }
    }
}

/// One settable control that the automation channel can inspect or set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FieldSpec {
    /// The frozen control id.
    pub(crate) id: &'static str,
    /// The frozen window id that holds the control.
    pub(crate) window: &'static str,
    /// The widget kind.
    pub(crate) kind: FieldKind,
}

/// Return every settable control, derived from the window registry by suffix.
#[must_use]
pub(crate) fn fields() -> Vec<FieldSpec> {
    WINDOWS
        .iter()
        .flat_map(|window| {
            window.controls.iter().filter_map(move |control| {
                field_kind(control).map(|kind| FieldSpec {
                    id: control,
                    window: window.window,
                    kind,
                })
            })
        })
        .collect()
}

/// Return the widget kind the control's frozen suffix names, or `None` when it is not settable.
fn field_kind(control: &str) -> Option<FieldKind> {
    if control.ends_with("_input") || control.ends_with("_picker") {
        Some(FieldKind::Text)
    } else if control.ends_with("_choice") {
        Some(FieldKind::Choice)
    } else if control.ends_with("_chk") {
        Some(FieldKind::Check)
    } else if control.ends_with("_spin") {
        Some(FieldKind::Spin)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
