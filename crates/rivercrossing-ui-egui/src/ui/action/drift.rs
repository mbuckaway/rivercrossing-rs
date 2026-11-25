// SPDX-License-Identifier: GPL-3.0-only

//! The ids-drift support: build the reviewed machine block from the compiled registry, read it back
//! from `design/docs-md/gui-windows.md`, and name the first mismatch.

use serde_json::{Map, Value, json};

use super::spec::{ACTIONS, ActionSpec, Shortcut, target_json};
use crate::ui::ids::{self, fields};

/// The reviewed document, compiled in so a rename cannot drift.
pub(crate) const GUI_WINDOWS_DOC: &str =
    include_str!("../../../../../design/docs-md/gui-windows.md");

/// The one fence tag the machine block uses.
const JSON_FENCE: &str = "```json";

/// The closing fence tag.
const FENCE_END: &str = "```";

/// Build the reviewed machine block from the frozen registry.
///
/// The block holds the 30-window control map, the 53 actions in table order, the automation-only
/// command ids and the derived settable fields.
#[must_use]
pub(crate) fn registry_block() -> Value {
    json!({
        "windows": windows_json(),
        "actions": actions_json(),
        "commands": [ids::APP_QUIT],
        "fields": fields_json(),
    })
}

/// Return the one fenced `json` machine block from the document.
///
/// # Errors
///
/// Returns an error when the document holds no `json` fence, more than one `json` fence, or a block
/// that is not valid JSON.
pub(crate) fn machine_block(doc: &str) -> Result<Value, String> {
    let fences = doc.lines().filter(|line| line.trim() == JSON_FENCE).count();
    if fences != 1 {
        return Err(format!(
            "the document must hold exactly one {JSON_FENCE} fence, found {fences}"
        ));
    }
    let body: Vec<&str> = doc
        .lines()
        .skip_while(|line| line.trim() != JSON_FENCE)
        .skip(1)
        .take_while(|line| line.trim() != FENCE_END)
        .collect();
    serde_json::from_str(&body.join("\n"))
        .map_err(|error| format!("the machine block is not valid JSON: {error}"))
}

/// Return the text between two headings.
///
/// # Errors
///
/// Returns an error when the document lacks either heading.
pub(crate) fn section<'a>(doc: &'a str, start: &str, end: &str) -> Result<&'a str, String> {
    let from = doc
        .find(start)
        .ok_or_else(|| format!("the document has no {start:?} heading"))?;
    let rest = doc
        .get(from..)
        .ok_or_else(|| format!("the {start:?} heading is not on a UTF-8 boundary"))?;
    let to = rest
        .find(end)
        .ok_or_else(|| format!("the document has no {end:?} heading after {start:?}"))?;
    rest.get(..to)
        .ok_or_else(|| format!("the {end:?} heading is not on a UTF-8 boundary"))
}

/// Compare an object in the document with the registry, naming the first mismatch.
///
/// # Errors
///
/// Returns an error naming the first entry that differs, or the entry count when the lengths differ.
pub(crate) fn object_matches(
    label: &str,
    doc: Option<&Value>,
    registry: Option<&Value>,
) -> Result<(), String> {
    let doc = doc
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{label}: the document entry is not an object"))?;
    let registry = registry
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{label}: the registry entry is not an object"))?;
    if doc.len() != registry.len() {
        return Err(format!(
            "{label}: entry count {} does not match the registry count {}",
            doc.len(),
            registry.len()
        ));
    }
    match registry
        .iter()
        .find(|(key, value)| doc.get(*key) != Some(*value))
    {
        Some((key, value)) => {
            let found = doc
                .get(key.as_str())
                .map_or_else(|| "missing".to_owned(), ToString::to_string);
            Err(format!(
                "{label}: key {key} expected {value}, found {found}"
            ))
        }
        None => Ok(()),
    }
}

/// Compare an array in the document with the registry in order, naming the first mismatch.
///
/// # Errors
///
/// Returns an error naming the first entry that differs, or the entry count when the lengths differ.
pub(crate) fn array_matches(
    label: &str,
    doc: Option<&Value>,
    registry: Option<&Value>,
) -> Result<(), String> {
    let doc = doc
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{label}: the document entry is not an array"))?;
    let registry = registry
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{label}: the registry entry is not an array"))?;
    if doc.len() != registry.len() {
        return Err(format!(
            "{label}: entry count {} does not match the registry count {}",
            doc.len(),
            registry.len()
        ));
    }
    match registry
        .iter()
        .zip(doc)
        .enumerate()
        .find(|(_, (expected, found))| expected != found)
    {
        Some((index, (expected, found))) => Err(format!(
            "{label}: entry {index} expected {expected}, found {found}"
        )),
        None => Ok(()),
    }
}

/// Return the window id to control-ids map.
fn windows_json() -> Value {
    let map: Map<String, Value> = ids::WINDOWS
        .iter()
        .map(|window| (window.window.to_owned(), json!(window.controls)))
        .collect();
    Value::Object(map)
}

/// Return the action rows in table order.
fn actions_json() -> Value {
    Value::Array(ACTIONS.iter().map(action_json).collect())
}

/// Return one action row as the machine block writes it.
fn action_json(row: &ActionSpec) -> Value {
    json!({
        "id": row.id.as_str(),
        "label": row.label,
        "shortcut": shortcut_json(row.shortcut),
        "target": target_json(row.target),
    })
}

/// Return the shortcut object, or `null` when the row has none.
fn shortcut_json(shortcut: Option<Shortcut>) -> Value {
    match shortcut {
        Some(shortcut) => json!({ "key": shortcut.key.as_str(), "command": shortcut.command }),
        None => Value::Null,
    }
}

/// Return the derived settable fields in registry order.
fn fields_json() -> Value {
    Value::Array(
        fields()
            .into_iter()
            .map(|field| {
                json!({
                    "id": field.id,
                    "window": field.window,
                    "kind": field.kind.as_str(),
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
