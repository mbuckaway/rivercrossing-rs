// SPDX-License-Identifier: GPL-3.0-only

//! The v1 UI scenario and observation model shared with the Python parity driver.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::selector::{Selector, action_target};

/// A recorded UI scenario: a version, an id and the steps to run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    /// The scenario format version; `1` is the only one this build reads.
    pub version: u32,
    /// The scenario's stable name.
    pub id: String,
    /// The window size to run at, in points, when the scenario fixes one.
    pub size: Option<[u32; 2]>,
    /// The steps, run in order.
    pub steps: Vec<Step>,
}

/// One step of a scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    /// Send a command through the automation channel.
    Command {
        /// The frozen action id.
        action: String,
        /// The command parameters; null asks the probe for its text field's contents.
        #[serde(default)]
        params: Value,
    },
    /// Click the node the selector names.
    Click {
        /// The node to click.
        selector: Selector,
    },
    /// Type text into the node the selector names.
    Type {
        /// The node that takes the text.
        selector: Selector,
        /// The text to enter.
        text: String,
    },
    /// Press a key, with optional modifiers.
    Key {
        /// The key name, such as `Enter`.
        key: String,
        /// The modifier names held during the press, such as `ctrl`.
        #[serde(default)]
        modifiers: Vec<String>,
    },
    /// Record a screenshot under this name.
    Snapshot {
        /// The screenshot's name.
        name: String,
    },
    /// Assert the accessible name and value of the node the selector names.
    Expect {
        /// The node to check.
        selector: Selector,
        /// The expected accessible name, when the step checks one.
        name: Option<String>,
        /// The expected value, when the step checks one.
        value: Option<String>,
    },
}

impl Step {
    /// Check this `Expect` step against a settled access tree.
    ///
    /// # Errors
    ///
    /// Return an error when the step is not an `Expect`, the selector is unknown or window-scoped,
    /// it matches no node or more than one, or the node's name or value differs from the expected one.
    pub fn evaluate(&self, tree: &WireTree) -> Result<(), String> {
        let Self::Expect {
            selector,
            name,
            value,
        } = self
        else {
            return Err("only an `expect` step can be evaluated".to_owned());
        };
        let node = matched_node(selector, tree)?;
        for (field, found, expected) in [
            ("name", node.name.as_deref(), name.as_deref()),
            ("value", node.value.as_deref(), value.as_deref()),
        ] {
            let Some(expected) = expected else {
                continue;
            };
            if found != Some(expected) {
                return Err(format!(
                    "node {} {field} is {}, expected {:?}",
                    node.id,
                    shown(found),
                    expected
                ));
            }
        }
        Ok(())
    }
}

/// One settled view: the app, the viewport and the flat access tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// The app that produced the observation.
    pub app: String,
    /// The observation format version; `1` is the only one this build reads.
    pub version: u32,
    /// The viewport size in points.
    pub viewport: [u32; 2],
    /// The flat access tree of the root window.
    pub tree: WireTree,
    /// The screenshot names written beside the observation.
    pub screenshots: Vec<String>,
}

/// The flat access tree, as craft's `ui.accessTree` reply carries it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireTree {
    /// The nodes, each naming its children by id.
    pub nodes: Vec<Node>,
    /// The focused node's id, when one is focused.
    pub focus: Option<u64>,
    /// The root node's id, when the tree reports one.
    pub root: Option<u64>,
}

/// One flat node: its id, role, optional name and value, and its children's ids.
///
/// A craft reply also carries `textSelection` on some nodes; the model ignores it on purpose.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// The AccessKit node id.
    pub id: u64,
    /// The AccessKit role spelling, as `format!("{:?}", role)` prints it.
    pub role: String,
    /// The accessible name, when the node has one.
    pub name: Option<String>,
    /// The accessible value, when the node has one.
    pub value: Option<String>,
    /// The ids of the node's children, in their declared order.
    pub children: Vec<u64>,
}

/// The one node `selector` names in `tree`.
fn matched_node<'a>(selector: &Selector, tree: &'a WireTree) -> Result<&'a Node, String> {
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
    let matching: Vec<&Node> = tree
        .nodes
        .iter()
        .filter(|node| node.role.eq_ignore_ascii_case(role) && node.name.as_deref() == Some(label))
        .collect();
    match matching.as_slice() {
        [] => Err(format!("no node matches role `{role}` and label `{label}`")),
        [node] => Ok(*node),
        many => Err(format!(
            "{} nodes match role `{role}` and label `{label}`; a window scope is required",
            many.len()
        )),
    }
}

/// Show an optional string as the error message prints it.
fn shown(value: Option<&str>) -> String {
    match value {
        Some(text) => format!("{text:?}"),
        None => "none".to_owned(),
    }
}

#[cfg(test)]
mod tests;
