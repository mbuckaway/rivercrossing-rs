// SPDX-License-Identifier: GPL-3.0-only

//! The frozen dialog contracts: the eight dialogs EPIC-12 Phase 3 opens, and their button rules.

/// The stock buttons a dialog can carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Button {
    /// The affirmative stock button.
    Ok,
    /// The safe stock button that Esc activates.
    Cancel,
    /// The stock button that dismisses an information dialog.
    Close,
    /// The destructive stock button.
    Delete,
    /// A custom button, by its frozen control id.
    Custom(&'static str),
}

impl Button {
    /// Return the label of the button; a custom button returns its frozen control id.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Cancel => "Cancel",
            Self::Close => "Close",
            Self::Delete => "Delete",
            Self::Custom(id) => id,
        }
    }
}

/// One dialog's frozen contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DialogSpec {
    /// The frozen window id, such as `delete_ride_dlg`.
    pub(crate) id: &'static str,
    /// The window title.
    pub(crate) title: &'static str,
    /// The button Enter activates.
    pub(crate) default: Button,
    /// The button Esc activates; always the safe one.
    pub(crate) cancel: Button,
    /// Whether the dialog confirms a destructive action.
    pub(crate) destructive: bool,
    /// The first field of a form dialog, which holds focus when the dialog opens.
    pub(crate) first_field: Option<&'static str>,
    /// The focusable buttons, in visual order from left to right.
    pub(crate) buttons: &'static [Button],
}

/// The dialogs EPIC-12 Phase 3 opens, in declaration order.
///
/// `resume_dlg` renders no cancel button; Esc still cancels through `Button::Cancel`, as the wx
/// dialog did. `delete_ride_dlg` is a destructive confirm, so it opens on Cancel and leaves
/// `first_field` empty; its typed-name field stays settable by its frozen control id.
pub(crate) const DIALOGS: &[DialogSpec] = &[
    DialogSpec {
        id: "ride_setup_dlg",
        title: "Ride Setup",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: Some("name_input"),
        buttons: &[Button::Ok, Button::Cancel],
    },
    DialogSpec {
        id: "start_blocked_dlg",
        title: "Cannot Start Ride",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: None,
        buttons: &[Button::Ok, Button::Cancel],
    },
    DialogSpec {
        id: "exit_running_dlg",
        title: "Ride Is Running",
        default: Button::Cancel,
        cancel: Button::Cancel,
        destructive: false,
        first_field: None,
        buttons: &[
            Button::Cancel,
            Button::Custom("finish_first_btn"),
            Button::Ok,
        ],
    },
    DialogSpec {
        id: "delete_ride_dlg",
        title: "Delete Ride",
        default: Button::Cancel,
        cancel: Button::Cancel,
        destructive: true,
        first_field: Some("confirm_name_input"),
        buttons: &[Button::Delete, Button::Cancel],
    },
    DialogSpec {
        id: "resume_dlg",
        title: "Resume Ride",
        default: Button::Custom("continue_btn"),
        cancel: Button::Cancel,
        destructive: false,
        first_field: None,
        buttons: &[
            Button::Custom("continue_btn"),
            Button::Custom("library_btn"),
        ],
    },
    DialogSpec {
        id: "set_start_dlg",
        title: "Set Start Time",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: Some("start_date_picker"),
        buttons: &[Button::Ok, Button::Cancel],
    },
    DialogSpec {
        id: "about_dlg",
        title: "About RiverCrossing",
        default: Button::Close,
        cancel: Button::Close,
        destructive: false,
        first_field: None,
        buttons: &[Button::Close],
    },
    DialogSpec {
        id: "shortcuts_dlg",
        title: "Keyboard Shortcuts",
        default: Button::Close,
        cancel: Button::Close,
        destructive: false,
        first_field: None,
        buttons: &[Button::Close],
    },
    DialogSpec {
        id: "rider_editor_dlg",
        title: "Rider Editor",
        default: Button::Close,
        cancel: Button::Close,
        destructive: false,
        first_field: None,
        buttons: &[Button::Close],
    },
    DialogSpec {
        id: "add_rider_dlg",
        title: "Add Rider",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: Some("first_name_input"),
        buttons: &[Button::Ok, Button::Cancel],
    },
    DialogSpec {
        id: "csv_preview_dlg",
        title: "Import Riders",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: None,
        buttons: &[Button::Ok, Button::Cancel],
    },
    DialogSpec {
        id: "team_editor_dlg",
        title: "Teams Editor",
        default: Button::Close,
        cancel: Button::Close,
        destructive: false,
        first_field: None,
        buttons: &[Button::Close],
    },
    DialogSpec {
        id: "add_team_dlg",
        title: "Add Team",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: Some("name_input"),
        buttons: &[Button::Ok, Button::Cancel],
    },
    DialogSpec {
        id: "rider_issues_dlg",
        title: "Rider Issues",
        default: Button::Close,
        cancel: Button::Close,
        destructive: false,
        first_field: None,
        buttons: &[Button::Close],
    },
    DialogSpec {
        id: "edit_crossing_dlg",
        title: "Edit Crossing",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: Some("plate_input"),
        buttons: &[Button::Ok, Button::Cancel],
    },
    DialogSpec {
        id: "manual_deal_dlg",
        title: "Deal Bonus Card",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: Some("plate_input"),
        buttons: &[Button::Ok, Button::Cancel],
    },
    DialogSpec {
        id: "dnf_confirm_dlg",
        title: "Mark DNF",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: Some("plate_input"),
        buttons: &[Button::Ok, Button::Cancel],
    },
    DialogSpec {
        id: "void_card_confirm_dlg",
        title: "Void Card",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: Some("reason_input"),
        buttons: &[Button::Ok, Button::Cancel],
    },
    DialogSpec {
        id: "ride_library_dlg",
        title: "Ride Library",
        default: Button::Ok,
        cancel: Button::Close,
        destructive: false,
        first_field: None,
        buttons: &[
            Button::Ok,
            Button::Custom("duplicate_btn"),
            Button::Delete,
            Button::Close,
        ],
    },
    DialogSpec {
        id: "audit_dlg",
        title: "Audit Trail",
        default: Button::Close,
        cancel: Button::Close,
        destructive: false,
        first_field: None,
        buttons: &[Button::Close],
    },
    DialogSpec {
        id: "settings_dlg",
        title: "Settings",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: None,
        buttons: &[Button::Ok, Button::Cancel],
    },
    DialogSpec {
        id: "selftest_dlg",
        title: "Evaluator Self-Test",
        default: Button::Close,
        cancel: Button::Close,
        destructive: false,
        first_field: None,
        buttons: &[Button::Close, Button::Custom("rerun_btn")],
    },
    DialogSpec {
        id: "crossing_detail_dlg",
        title: "Crossing Detail",
        default: Button::Ok,
        cancel: Button::Close,
        destructive: false,
        first_field: None,
        buttons: &[
            Button::Ok,
            Button::Custom("edit_btn"),
            Button::Custom("edit_time_btn"),
            Button::Custom("void_card_btn"),
            Button::Custom("delete_btn"),
        ],
    },
    DialogSpec {
        id: "crossing_number_dlg",
        title: "Plate",
        default: Button::Ok,
        cancel: Button::Cancel,
        destructive: false,
        first_field: Some("number_input"),
        buttons: &[Button::Ok, Button::Cancel],
    },
];

/// Return the contract of the dialog id, or `None` when the id is not a dialog.
#[must_use]
pub(crate) fn spec(id: &str) -> Option<&'static DialogSpec> {
    DIALOGS.iter().find(|dialog| dialog.id == id)
}

#[cfg(test)]
mod tests;
