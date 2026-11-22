// SPDX-License-Identifier: GPL-3.0-only

//! The layering rule: only the egui front end and the workspace apps may depend on the UI stack.

use serde_json::Value;
use thiserror::Error;

/// The dependencies that only the UI layer may declare.
const FORBIDDEN: [&str; 4] = ["egui", "eframe", "winit", "rfd"];

/// The package that may always declare a forbidden dependency.
const UI_PACKAGE: &str = "rivercrossing-ui-egui";

/// The normalised path segment of a workspace app.
const APPS_SEGMENT: &str = "/apps/";

/// A failure while reading the `cargo metadata` output.
#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum LayersError {
    /// The text is not JSON that `cargo metadata --format-version 1` writes.
    #[error("cargo metadata is not valid JSON: {0}")]
    InvalidJson(String),
    /// The JSON has no `packages` array.
    #[error("cargo metadata has no packages array")]
    MissingPackages,
}

/// Return each forbidden dependency that a workspace package declares.
///
/// Each line reads `<crate> -> <dependency>`, the list is sorted, and the list is empty
/// when the workspace obeys the rule. Every dependency kind counts. Only the packages
/// that `workspace_members` names take part, so a third-party crate that uses the UI
/// stack is ignored. A document without `workspace_members` is treated as one workspace.
pub(crate) fn violations(metadata_json: &str) -> Result<Vec<String>, LayersError> {
    let root: Value = serde_json::from_str(metadata_json)
        .map_err(|error| LayersError::InvalidJson(error.to_string()))?;
    let packages = root
        .get("packages")
        .and_then(Value::as_array)
        .ok_or(LayersError::MissingPackages)?;
    let members = workspace_members(&root);

    let mut found: Vec<String> = packages
        .iter()
        .filter(|package| is_member(package, &members))
        .filter(|package| !is_exempt(package))
        .flat_map(offending_dependencies)
        .collect();
    found.sort();
    Ok(found)
}

/// Return the package ids of `workspace_members`, or an empty list when the key is absent.
fn workspace_members(root: &Value) -> Vec<&str> {
    root.get("workspace_members")
        .and_then(Value::as_array)
        .map(|members| members.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Return true when the package is a workspace member.
///
/// An empty list means that the document names no members, so every package is treated
/// as one.
fn is_member(package: &Value, members: &[&str]) -> bool {
    members.is_empty() || members.contains(&text_field(package, "id"))
}

/// Return true when the package may declare a forbidden dependency.
fn is_exempt(package: &Value) -> bool {
    if text_field(package, "name") == UI_PACKAGE {
        return true;
    }
    text_field(package, "manifest_path")
        .replace('\\', "/")
        .contains(APPS_SEGMENT)
}

/// Return each forbidden dependency of one package as a report line.
fn offending_dependencies(package: &Value) -> Vec<String> {
    let Some(dependencies) = package.get("dependencies").and_then(Value::as_array) else {
        return Vec::new();
    };
    let package_name = text_field(package, "name");
    dependencies
        .iter()
        .filter_map(|dependency| forbidden_line(package_name, dependency))
        .collect()
}

/// Return the report line for a forbidden dependency, or `None` for an allowed one.
fn forbidden_line(package_name: &str, dependency: &Value) -> Option<String> {
    let dependency_name = text_field(dependency, "name");
    if !FORBIDDEN.contains(&dependency_name) {
        return None;
    }
    Some(format!("{package_name} -> {dependency_name}"))
}

/// Return a string field of a JSON value, or the empty string when the field is absent.
fn text_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

#[cfg(test)]
mod tests;
