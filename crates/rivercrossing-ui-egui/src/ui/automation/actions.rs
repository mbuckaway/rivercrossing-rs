// SPDX-License-Identifier: GPL-3.0-only

//! The pure automation actions: `ui.command`, `ui.inspect`, `ui.set`, `ui.dialog.*` and the click
//! selector's action lookup, over the §15 action registry and the dialog host.

use serde_json::{Value, json};

use crate::ui::action::effect::DispatchError;
use crate::ui::action::id::ActionId;
use crate::ui::action::spec;
use crate::ui::dialogs::spec::{self as dialog_spec, Button};
use crate::ui::dialogs::stack::{DialogError, DialogEvent};
use crate::ui::ids;
use crate::ui::state::AppState;

/// The automation answer when no dialog is open.
const NO_DIALOG: &str = "no dialog is open";

/// Run `ui.command`: dispatch the named action, or answer `app_quit` with `{"bye": true}`.
///
/// # Errors
///
/// Returns a message when the name is not a frozen action id, when the §15 gate refuses the action
/// in this ride state, or when the action opens a window this release does not build.
pub(crate) fn command(state: &mut AppState, name: &str) -> Result<Value, String> {
    if name == ids::APP_QUIT {
        return Ok(json!({ "bye": true }));
    }
    let Some(id) = action_id(name) else {
        return Err(format!("unknown command `{name}`"));
    };
    match state.act(id) {
        Ok(_) => Ok(json!({ "ok": true })),
        Err(DispatchError::Disabled(_)) => {
            Err(format!("command `{name}` is disabled in this state"))
        }
        Err(_) => Err(format!("command `{name}` is not available in this release")),
    }
}

/// Read `ui.inspect`: return the named top-dialog field as `{"value": <string|null>}`.
///
/// # Errors
///
/// Returns a message when `field` or `window` is not a string, when no dialog is open, when
/// `window` does not name the top dialog, or when the dialog does not declare the field.
pub(crate) fn inspect(state: &AppState, params: &Value) -> Result<Value, String> {
    let field = param_str(params, "field", "ui.inspect")?;
    check_window(state, params, "ui.inspect")?;
    let top = state.dialogs.top().ok_or_else(|| NO_DIALOG.to_owned())?;
    if !field_known(top.id, field) {
        return Err(format!("unknown field `{field}`"));
    }
    Ok(json!({ "value": top.fields.get(field) }))
}

/// Apply `ui.set`: write the string `value` into the named top-dialog field.
///
/// # Errors
///
/// Returns a message when `field` or `value` is not a string, when no dialog is open, when
/// `window` does not name the top dialog, or when the dialog does not declare the field.
pub(crate) fn set(state: &mut AppState, params: &Value) -> Result<Value, String> {
    let field = param_str(params, "field", "ui.set")?;
    let value = param_str(params, "value", "ui.set")?;
    check_window(state, params, "ui.set")?;
    set_top_field(state, field, value)?;
    Ok(json!({ "ok": true }))
}

/// Apply a `ui.dialog.*` method: a top-dialog field write, Enter (`confirm`) or Esc (`cancel`).
///
/// # Errors
///
/// Returns a message for an unknown method, a `field` or `value` that is not a string, no open
/// dialog, or a field the dialog does not declare.
pub(crate) fn dialog(state: &mut AppState, method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "ui.dialog.set" => {
            let field = param_str(params, "field", method)?;
            let value = param_str(params, "value", method)?;
            set_top_field(state, field, value)?;
            Ok(json!({ "ok": true }))
        }
        "ui.dialog.confirm" => confirm(state),
        "ui.dialog.cancel" => cancel(state),
        other => Err(format!("unknown method `{other}`")),
    }
}

/// Return the AccessKit role and label of the on-screen console button that the action id names.
///
/// Only the console's action buttons are clickable by action id. The plate field is addressed by
/// its field id (`plate_input`), and every other action is menu-only, so drive it with
/// `ui.command`.
pub(crate) fn action_target(action: &str) -> Option<(&'static str, &'static str)> {
    match action {
        "mi_start_ride" => Some(("Button", "Start Ride")),
        "mi_stop_ride" => Some(("Button", "Stop Ride")),
        "mi_undo_crossing" => Some(("Button", "Undo")),
        _ => None,
    }
}

/// Return the frozen action id that `name` spells, or `None` when the §15 table has no such row.
///
/// An automation name arrives as a `&str`, which `ActionId::from_name` cannot wrap, so the lookup
/// runs against the frozen table and yields the canonical id.
fn action_id(name: &str) -> Option<ActionId> {
    spec::ACTIONS
        .iter()
        .map(|row| row.id)
        .find(|id| id.as_str() == name)
}

/// Activate the top dialog's Enter button and report the event.
///
/// A native confirmation is the top dialog while no [`crate::ui::dialogs::stack::DialogStack`]
/// dialog is open, so the same method confirms it.
fn confirm(state: &mut AppState) -> Result<Value, String> {
    if state.dialogs.is_empty() {
        return if state.confirm_native() {
            Ok(json!({
                "event": "confirmed",
                "button": "OK",
                "opener": null,
            }))
        } else {
            Err(NO_DIALOG.to_owned())
        };
    }
    if let Some(outcome) = state.submit_top_form() {
        // The form's OK commits; a refusal keeps it open and returns the message.
        outcome?;
        return Ok(json!({
            "event": "confirmed",
            "button": "OK",
            "opener": null,
        }));
    }
    match state.dialogs.confirm() {
        Ok(DialogEvent::Confirmed { button, opener, .. }) => Ok(json!({
            "event": "confirmed",
            "button": button.as_str(),
            "opener": opener,
        })),
        Ok(_) | Err(_) => Err(NO_DIALOG.to_owned()),
    }
}

/// Activate the top dialog's Esc button and report the event.
///
/// A native confirmation is the top dialog while no [`crate::ui::dialogs::stack::DialogStack`]
/// dialog is open, so the same method cancels it.
fn cancel(state: &mut AppState) -> Result<Value, String> {
    if state.dialogs.is_empty() {
        return if state.cancel_native() {
            Ok(json!({
                "event": "cancelled",
                "button": "Cancel",
                "opener": null,
            }))
        } else {
            Err(NO_DIALOG.to_owned())
        };
    }
    match state.dialogs.cancel() {
        Ok(DialogEvent::Cancelled { id, opener }) => Ok(json!({
            "event": "cancelled",
            "button": cancel_button(id),
            "opener": opener,
        })),
        Ok(_) | Err(_) => Err(NO_DIALOG.to_owned()),
    }
}

/// Return the button that Esc activates for the dialog that closed.
fn cancel_button(id: &'static str) -> &'static str {
    dialog_spec::spec(id).map_or(Button::Cancel.as_str(), |dialog| dialog.cancel.as_str())
}

/// Write one field on the top dialog.
fn set_top_field(state: &mut AppState, field: &str, value: &str) -> Result<(), String> {
    match state.dialogs.set_field(field, value) {
        Ok(()) => Ok(()),
        Err(DialogError::UnknownField) => Err(format!("unknown field `{field}`")),
        Err(_) => Err(NO_DIALOG.to_owned()),
    }
}

/// Reject a `window` name that is not the top dialog.
fn check_window(state: &AppState, params: &Value, method: &str) -> Result<(), String> {
    let Some(window) = optional_str(params, "window", method)? else {
        return Ok(());
    };
    let top = state.dialogs.top().ok_or_else(|| NO_DIALOG.to_owned())?;
    if window == top.id {
        Ok(())
    } else {
        Err(format!("no dialog `{window}` is open"))
    }
}

/// Return the string `key` of the request parameters.
fn param_str<'a>(params: &'a Value, key: &str, method: &str) -> Result<&'a str, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{method} needs a string `{key}`"))
}

/// Return the string `key` of the request parameters, or `None` when it is absent.
fn optional_str<'a>(params: &'a Value, key: &str, method: &str) -> Result<Option<&'a str>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(Some)
            .ok_or_else(|| format!("{method} needs a string `{key}`")),
    }
}

/// Return whether the window declares the control as a settable field.
fn field_known(window: &str, field: &str) -> bool {
    ids::fields()
        .iter()
        .any(|entry| entry.window == window && entry.id == field)
}

#[cfg(test)]
mod tests;
