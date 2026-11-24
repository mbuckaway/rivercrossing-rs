// SPDX-License-Identifier: GPL-3.0-only

//! The `Selector` contract and its resolution to an on-screen rectangle.

use craft::egui::Rect;
use serde::Deserialize;

/// A stable handle on one UI node: a frozen action id first, an AccessKit role and label second.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Selector {
    /// A frozen action id, such as `probe_echo`.
    Action {
        /// The frozen action id.
        action: String,
    },
    /// An AccessKit role and label, optionally scoped to a window.
    RoleAndLabel {
        /// The AccessKit role spelling, compared case-insensitively.
        role: String,
        /// The exact accessible label.
        #[serde(alias = "name")]
        label: String,
        /// The window scope; unsupported while the live tree is root-only.
        #[serde(default)]
        window: Option<String>,
    },
}

/// One resolvable node: its AccessKit role, its label and its last-pass bounds in egui points.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// The AccessKit role spelling, as `format!("{:?}", role)` prints it.
    pub role: String,
    /// The accessible label.
    pub label: Option<String>,
    /// The node's bounds in egui points.
    pub bounds: Rect,
}

/// Return the bounds of the one node `selector` names.
///
/// # Errors
///
/// Return an error when the action id is unknown, the window scope is set, no node matches, or
/// more than one node matches without a window.
pub fn resolve(selector: &Selector, candidates: &[Candidate]) -> Result<Rect, String> {
    let (role, label) = match selector {
        Selector::Action { action } => {
            action_target(action).ok_or_else(|| format!("unknown action `{action}`"))?
        }
        Selector::RoleAndLabel { role, label, .. } => (role.as_str(), label.as_str()),
    };
    if let Selector::RoleAndLabel {
        window: Some(window),
        ..
    } = selector
    {
        return Err(format!(
            "window-scoped selectors are not supported at EPIC-10 (window `{window}`)"
        ));
    }
    let matching: Vec<&Candidate> = candidates
        .iter()
        .filter(|candidate| {
            candidate.role.eq_ignore_ascii_case(role) && candidate.label.as_deref() == Some(label)
        })
        .collect();
    match matching.as_slice() {
        [] => Err(format!("no node matches role `{role}` and label `{label}`")),
        [found] => Ok(found.bounds),
        many => Err(format!(
            "{} nodes match role `{role}` and label `{label}`; a window scope is required",
            many.len()
        )),
    }
}

/// The role and label of each probe action id at EPIC-10.
fn action_target(action: &str) -> Option<(&'static str, &'static str)> {
    match action {
        "probe_echo" => Some(("Button", "Probe Echo")),
        "probe_input" => Some(("TextInput", "Probe Input")),
        _ => None,
    }
}
