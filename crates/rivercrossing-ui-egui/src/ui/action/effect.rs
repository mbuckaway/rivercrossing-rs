// SPDX-License-Identifier: GPL-3.0-only

//! The effects a dispatched action produces, and the reasons a dispatch refuses an action.

use super::id::ActionId;
use super::spec::{CheckKind, CommandId, NativeDialog};

/// The effect a dispatch asks the app to perform; Phase 6 wires each effect to the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    /// Open the frozen window with this id.
    OpenWindow(&'static str),
    /// Run the named command.
    RunCommand(CommandId),
    /// Toggle the named check.
    ToggleCheck(CheckKind),
    /// Set the density preset to this stored spelling.
    SetDensity(&'static str),
    /// Set the text size to this stored spelling.
    SetTextSize(&'static str),
    /// Open the ride-setup window in edit mode, preloaded with the live ride.
    OpenEditRide,
    /// Raise a native dialog; on confirm the app runs `then`.
    OpenNative {
        /// The native dialog kind to raise.
        dialog: NativeDialog,
        /// The command the app runs once the dialog is confirmed.
        then: CommandId,
    },
}

/// Why a dispatch refused an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DispatchError {
    /// The id is not in the action table.
    UnknownAction(ActionId),
    /// The §15 gate refuses the action in the current ride state.
    Disabled(ActionId),
    /// The action opens a window this release does not build; carries the window id.
    NotImplemented(&'static str),
}
