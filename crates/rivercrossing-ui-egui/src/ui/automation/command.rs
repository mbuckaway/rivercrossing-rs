// SPDX-License-Identifier: GPL-3.0-only

//! The EPIC-10 debug probe command table.

use serde_json::{Value, json};

/// What a `ui.command` probe asks the app to do.
#[derive(Debug, PartialEq)]
pub(crate) enum ProbeOutcome {
    /// Reply with the request parameters under `echo`.
    Echo(Value),
    /// Reply with the probe field's text under `echo` and show it.
    EchoField,
    /// Reply with this value, then close the window.
    Quit(Value),
}

/// Resolve `name` and its parameters against the EPIC-10 probe table.
///
/// # Errors
///
/// Return an error naming the command when it is not in the table.
pub(crate) fn probe_command(name: &str, params: &Value) -> Result<ProbeOutcome, String> {
    match name {
        "probe_echo" if params.is_null() => Ok(ProbeOutcome::EchoField),
        "probe_echo" => Ok(ProbeOutcome::Echo(params.clone())),
        "probe_quit" => Ok(ProbeOutcome::Quit(json!({"bye": true}))),
        other => Err(format!("unknown command `{other}`")),
    }
}
