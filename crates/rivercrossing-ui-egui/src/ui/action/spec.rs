// SPDX-License-Identifier: GPL-3.0-only

//! The §15 action table: one row per menu item, with its label, shortcut and target.

use serde_json::{Value, json};

use super::id;
use super::id::ActionId;

/// The destination an enabled action reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    /// Open the frozen window with this id.
    Window(&'static str),
    /// Run the named command.
    Command(CommandId),
    /// Toggle the named check.
    Check(CheckKind),
    /// Set the text zoom to this percentage.
    Zoom(u16),
    /// A native confirm, danger or prompt; on confirm the app runs `then`.
    Native {
        /// The native dialog kind to raise.
        dialog: NativeDialog,
        /// The command the app runs once the dialog is confirmed.
        then: CommandId,
    },
}

/// A command the registry runs; Phase 6 wires each command to its presenter handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandId {
    /// Duplicate the open ride as a new draft.
    DuplicateRide,
    /// Reopen the finished ride for corrections.
    ReopenRide,
    /// Start the ride.
    StartRide,
    /// Stop the live ride.
    StopRide,
    /// Finish the ride.
    FinishRide,
    /// Undo the last crossing.
    UndoCrossing,
    /// Unload the ride from memory without deleting it.
    ClearRide,
    /// Write a timestamped database backup.
    BackupNow,
    /// Export the results page as HTML.
    ExportHtml,
    /// Export the results as PDF.
    ExportPdf,
    /// Export the podium poster as PDF.
    ExportPoster,
    /// Export the podium poster as HTML.
    ExportPosterHtml,
    /// Export a standings CSV.
    ExportCsv,
    /// Open the last HTML export.
    PreviewHtml,
    /// Open the last podium poster page export.
    PreviewPosterHtml,
    /// Open the last PDF export.
    PreviewPdf,
    /// Open the bundled user guide.
    OpenUserGuide,
    /// Quit the app.
    AppQuit,
}

/// A checkable publish or time-column option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CheckKind {
    /// Show lap and total times in the published results.
    PublishShowTimes,
    /// Show the laps leaderboard in the exports.
    LapsBoard,
    /// Show the fastest-time leaderboard in the exports.
    TimeBoard,
    /// Show the full field in the exports.
    FullField,
    /// Show every drawn card in the exports.
    AllCards,
    /// Show the DNS rows on a finished ride's results.
    ShowDnsRiders,
    /// Show the Total column on the console.
    ShowTotalTimes,
    /// Show the Lap time column on the console.
    ShowLapTime,
}

/// The native dialog kinds the command handlers can raise (R-85).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeDialog {
    /// A plain confirmation.
    Confirm,
    /// A destructive confirmation.
    Danger,
    /// A non-destructive question.
    Prompt,
    /// An error report.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the native error dialog is raised by the command handlers in Phase 6, not by a §15 row"
        )
    )]
    Error,
}

/// The keyboard key a shortcut uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    /// The `Z` key.
    Z,
    /// The `F1` key.
    F1,
    /// The `F5` key.
    F5,
}

impl CommandId {
    /// Return the stable `snake_case` name, as the machine block spells it.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateRide => "duplicate_ride",
            Self::ReopenRide => "reopen_ride",
            Self::StartRide => "start_ride",
            Self::StopRide => "stop_ride",
            Self::FinishRide => "finish_ride",
            Self::UndoCrossing => "undo_crossing",
            Self::ClearRide => "clear_ride",
            Self::BackupNow => "backup_now",
            Self::ExportHtml => "export_html",
            Self::ExportPdf => "export_pdf",
            Self::ExportPoster => "export_poster",
            Self::ExportPosterHtml => "export_poster_html",
            Self::ExportCsv => "export_csv",
            Self::PreviewHtml => "preview_html",
            Self::PreviewPosterHtml => "preview_poster_html",
            Self::PreviewPdf => "preview_pdf",
            Self::OpenUserGuide => "open_user_guide",
            Self::AppQuit => "app_quit",
        }
    }
}

impl CheckKind {
    /// Return the stable `snake_case` name, as the machine block spells it.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::PublishShowTimes => "publish_show_times",
            Self::LapsBoard => "laps_board",
            Self::TimeBoard => "time_board",
            Self::FullField => "full_field",
            Self::AllCards => "all_cards",
            Self::ShowDnsRiders => "show_dns_riders",
            Self::ShowTotalTimes => "show_total_times",
            Self::ShowLapTime => "show_lap_time",
        }
    }
}

impl NativeDialog {
    /// Return the stable name, as the machine block spells it.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Confirm => "confirm",
            Self::Danger => "danger",
            Self::Prompt => "prompt",
            Self::Error => "error",
        }
    }
}

impl Key {
    /// Return the lowercase key name, as the machine block spells it.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Z => "z",
            Self::F1 => "f1",
            Self::F5 => "f5",
        }
    }
}

/// Return the target as the machine block writes it: one typed object per target kind.
#[must_use]
pub(crate) fn target_json(target: Target) -> Value {
    match target {
        Target::Window(window) => json!({ "kind": "window", "window": window }),
        Target::Command(command) => json!({ "kind": "command", "command": command.as_str() }),
        Target::Check(check) => json!({ "kind": "check", "check": check.as_str() }),
        Target::Zoom(percent) => json!({ "kind": "zoom", "percent": percent }),
        Target::Native { dialog, then } => {
            json!({ "kind": "native", "dialog": dialog.as_str(), "then": then.as_str() })
        }
    }
}

/// A documented keyboard shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shortcut {
    /// The key, with the command modifier when `command` is set.
    pub(crate) key: Key,
    /// Whether the command modifier (Ctrl on Linux and Windows, Command on macOS) applies.
    pub(crate) command: bool,
}

/// One §15 row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ActionSpec {
    /// The frozen action id.
    pub(crate) id: ActionId,
    /// The menu label.
    pub(crate) label: &'static str,
    /// The documented shortcut, when the documents state one.
    pub(crate) shortcut: Option<Shortcut>,
    /// The destination.
    pub(crate) target: Target,
}

/// The 53 actions, in §15 menu order.
pub(crate) const ACTIONS: &[ActionSpec] = &[
    // File
    ActionSpec {
        id: id::MI_OPEN_LIBRARY,
        label: "Ride Library",
        shortcut: None,
        target: Target::Window("ride_library_dlg"),
    },
    ActionSpec {
        id: id::MI_DUPLICATE_RIDE,
        label: "Duplicate Ride…",
        shortcut: None,
        target: Target::Native {
            dialog: NativeDialog::Prompt,
            then: CommandId::DuplicateRide,
        },
    },
    ActionSpec {
        id: id::MI_IMPORT_CSV,
        label: "Import Riders CSV…",
        shortcut: None,
        target: Target::Window("csv_preview_dlg"),
    },
    ActionSpec {
        id: id::MI_EXPORT_CSV,
        label: "Export Riders CSV…",
        shortcut: None,
        target: Target::Command(CommandId::ExportCsv),
    },
    ActionSpec {
        id: id::MI_BACKUP_NOW,
        label: "Back Up Database…",
        shortcut: None,
        target: Target::Command(CommandId::BackupNow),
    },
    ActionSpec {
        id: id::MI_SIMULATION,
        label: "Simulation…",
        shortcut: None,
        target: Target::Window("simulation_dlg"),
    },
    ActionSpec {
        id: id::WX_ID_PREFERENCES,
        label: "Settings…",
        shortcut: None,
        target: Target::Window("settings_dlg"),
    },
    ActionSpec {
        id: id::WX_ID_EXIT,
        label: "Exit",
        shortcut: None,
        target: Target::Native {
            dialog: NativeDialog::Confirm,
            then: CommandId::AppQuit,
        },
    },
    // Ride
    ActionSpec {
        id: id::MI_NEW_RIDE,
        label: "New Ride…",
        shortcut: None,
        target: Target::Window("ride_setup_dlg"),
    },
    ActionSpec {
        id: id::MI_EDIT_RIDE,
        label: "Edit Ride…",
        shortcut: None,
        target: Target::Window("ride_setup_dlg"),
    },
    ActionSpec {
        id: id::MI_START_RIDE,
        label: "Start Ride",
        shortcut: None,
        target: Target::Command(CommandId::StartRide),
    },
    ActionSpec {
        id: id::MI_STOP_RIDE,
        label: "Stop Ride…",
        shortcut: None,
        target: Target::Native {
            dialog: NativeDialog::Confirm,
            then: CommandId::StopRide,
        },
    },
    ActionSpec {
        id: id::MI_SET_START_TIME,
        label: "Set Start Time…",
        shortcut: None,
        target: Target::Window("set_start_dlg"),
    },
    ActionSpec {
        id: id::MI_FINISH_RIDE,
        label: "Finish Ride…",
        shortcut: None,
        target: Target::Native {
            dialog: NativeDialog::Danger,
            then: CommandId::FinishRide,
        },
    },
    ActionSpec {
        id: id::MI_REOPEN_RIDE,
        label: "Reopen Ride",
        shortcut: None,
        target: Target::Native {
            dialog: NativeDialog::Prompt,
            then: CommandId::ReopenRide,
        },
    },
    ActionSpec {
        id: id::MI_AUDIT_TRAIL,
        label: "Audit Trail…",
        shortcut: None,
        target: Target::Window("audit_dlg"),
    },
    ActionSpec {
        id: id::MI_CLEAR_RIDE,
        label: "Clear Ride…",
        shortcut: None,
        target: Target::Native {
            dialog: NativeDialog::Danger,
            then: CommandId::ClearRide,
        },
    },
    // Riders
    ActionSpec {
        id: id::MI_RIDER_EDITOR,
        label: "Rider Editor",
        shortcut: None,
        target: Target::Window("rider_editor_dlg"),
    },
    ActionSpec {
        id: id::MI_TEAM_EDITOR,
        label: "Teams Editor",
        shortcut: None,
        target: Target::Window("team_editor_dlg"),
    },
    ActionSpec {
        id: id::MI_CHECK_RIDER_ISSUES,
        label: "Check for Rider Issues…",
        shortcut: None,
        target: Target::Window("rider_issues_dlg"),
    },
    ActionSpec {
        id: id::MI_MARK_DNF,
        label: "Mark DNF…",
        shortcut: None,
        target: Target::Window("dnf_confirm_dlg"),
    },
    // Cards
    ActionSpec {
        id: id::MI_UNDO_CROSSING,
        label: "Undo Last Crossing",
        shortcut: Some(Shortcut {
            key: Key::Z,
            command: true,
        }),
        target: Target::Native {
            dialog: NativeDialog::Danger,
            then: CommandId::UndoCrossing,
        },
    },
    ActionSpec {
        id: id::MI_ADD_CROSSING_AT,
        label: "Add Crossing at Time…",
        shortcut: None,
        target: Target::Window("edit_crossing_dlg"),
    },
    ActionSpec {
        id: id::MI_DEAL_MANUAL,
        label: "Deal Bonus Card…",
        shortcut: None,
        target: Target::Window("manual_deal_dlg"),
    },
    // Results
    ActionSpec {
        id: id::MI_STANDINGS,
        label: "Standings",
        shortcut: Some(Shortcut {
            key: Key::F5,
            command: false,
        }),
        target: Target::Window("results_dlg"),
    },
    ActionSpec {
        id: id::MI_EXPORT_HTML,
        label: "Export HTML…",
        shortcut: None,
        target: Target::Command(CommandId::ExportHtml),
    },
    ActionSpec {
        id: id::MI_EXPORT_PDF,
        label: "Export PDF…",
        shortcut: None,
        target: Target::Command(CommandId::ExportPdf),
    },
    ActionSpec {
        id: id::MI_EXPORT_POSTER,
        label: "Podium Poster PDF…",
        shortcut: None,
        target: Target::Command(CommandId::ExportPoster),
    },
    ActionSpec {
        id: id::MI_EXPORT_POSTER_HTML,
        label: "Podium Poster HTML…",
        shortcut: None,
        target: Target::Command(CommandId::ExportPosterHtml),
    },
    ActionSpec {
        id: id::MI_EXPORT_RESULTS_CSV,
        label: "Export Standings CSV…",
        shortcut: None,
        target: Target::Command(CommandId::ExportCsv),
    },
    ActionSpec {
        id: id::MI_PREVIEW_HTML_BROWSER,
        label: "Preview HTML in Browser",
        shortcut: None,
        target: Target::Command(CommandId::PreviewHtml),
    },
    ActionSpec {
        id: id::MI_PREVIEW_POSTER_HTML_BROWSER,
        label: "Preview Podium Poster HTML in Browser",
        shortcut: None,
        target: Target::Command(CommandId::PreviewPosterHtml),
    },
    ActionSpec {
        id: id::MI_PREVIEW_PDF_BROWSER,
        label: "Preview PDF in Browser",
        shortcut: None,
        target: Target::Command(CommandId::PreviewPdf),
    },
    ActionSpec {
        id: id::MI_PUBLISH_WORDPRESS,
        label: "Publish to WordPress…",
        shortcut: None,
        target: Target::Window("publish_wordpress_dlg"),
    },
    ActionSpec {
        id: id::MI_SHOW_TIMES,
        label: "Show lap & total times",
        shortcut: None,
        target: Target::Check(CheckKind::PublishShowTimes),
    },
    ActionSpec {
        id: id::MI_LAPS_BOARD,
        label: "Laps leaderboard",
        shortcut: None,
        target: Target::Check(CheckKind::LapsBoard),
    },
    ActionSpec {
        id: id::MI_TIME_BOARD,
        label: "Fastest-time leaderboard",
        shortcut: None,
        target: Target::Check(CheckKind::TimeBoard),
    },
    ActionSpec {
        id: id::MI_FULL_FIELD,
        label: "Full field",
        shortcut: None,
        target: Target::Check(CheckKind::FullField),
    },
    ActionSpec {
        id: id::MI_ALL_CARDS,
        label: "All cards drawn",
        shortcut: None,
        target: Target::Check(CheckKind::AllCards),
    },
    ActionSpec {
        id: id::MI_SHOW_DNS_RIDERS,
        label: "Show DNS Riders",
        shortcut: None,
        target: Target::Check(CheckKind::ShowDnsRiders),
    },
    // View
    ActionSpec {
        id: id::MI_SHOW_TOTAL_TIMES,
        label: "Show Total Times on Crossings Panel",
        shortcut: None,
        target: Target::Check(CheckKind::ShowTotalTimes),
    },
    ActionSpec {
        id: id::MI_SHOW_LAP_TIME,
        label: "Show Lap Time on Crossings Panel",
        shortcut: None,
        target: Target::Check(CheckKind::ShowLapTime),
    },
    ActionSpec {
        id: id::MI_ZOOM_90,
        label: "90%",
        shortcut: None,
        target: Target::Zoom(90),
    },
    ActionSpec {
        id: id::MI_ZOOM_100,
        label: "100%",
        shortcut: None,
        target: Target::Zoom(100),
    },
    ActionSpec {
        id: id::MI_ZOOM_110,
        label: "110%",
        shortcut: None,
        target: Target::Zoom(110),
    },
    ActionSpec {
        id: id::MI_ZOOM_120,
        label: "120%",
        shortcut: None,
        target: Target::Zoom(120),
    },
    ActionSpec {
        id: id::MI_ZOOM_130,
        label: "130%",
        shortcut: None,
        target: Target::Zoom(130),
    },
    ActionSpec {
        id: id::MI_ZOOM_140,
        label: "140%",
        shortcut: None,
        target: Target::Zoom(140),
    },
    ActionSpec {
        id: id::MI_ZOOM_150,
        label: "150%",
        shortcut: None,
        target: Target::Zoom(150),
    },
    // Help
    ActionSpec {
        id: id::MI_USER_GUIDE,
        label: "User Guide",
        shortcut: Some(Shortcut {
            key: Key::F1,
            command: false,
        }),
        target: Target::Command(CommandId::OpenUserGuide),
    },
    ActionSpec {
        id: id::MI_SHORTCUTS,
        label: "Keyboard Shortcuts",
        shortcut: None,
        target: Target::Window("shortcuts_dlg"),
    },
    ActionSpec {
        id: id::MI_SELFTEST,
        label: "Run Evaluator Self-test",
        shortcut: None,
        target: Target::Window("selftest_dlg"),
    },
    ActionSpec {
        id: id::WX_ID_ABOUT,
        label: "About RiverCrossing",
        shortcut: None,
        target: Target::Window("about_dlg"),
    },
];

/// Return the action row for the id, or `None` when the id is not in the table.
#[must_use]
pub(crate) fn spec(id: ActionId) -> Option<&'static ActionSpec> {
    ACTIONS.iter().find(|row| row.id == id)
}

#[cfg(test)]
mod tests;
