// SPDX-License-Identifier: GPL-3.0-only

//! The drift tests: the reviewed document against the compiled registry.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::{
    GUI_WINDOWS_DOC, array_matches, machine_block, object_matches, registry_block, section,
};
use crate::ui::action::spec::ACTIONS;
use crate::ui::ids;

#[test]
fn test_ids_drift_registry_matches_gui_windows() -> Result<(), String> {
    // Arrange: the registry block is built from ids::WINDOWS, ACTIONS and fields().
    let doc = machine_block(GUI_WINDOWS_DOC)?;
    let registry = registry_block();

    // Act and assert
    assert_eq!(
        object_matches("windows", doc.get("windows"), registry.get("windows")),
        Ok(())
    );
    assert_eq!(
        array_matches("actions", doc.get("actions"), registry.get("actions")),
        Ok(())
    );
    assert_eq!(
        doc.get("commands"),
        registry.get("commands"),
        "the commands array"
    );
    assert_eq!(
        array_matches("fields", doc.get("fields"), registry.get("fields")),
        Ok(())
    );
    Ok(())
}

#[test]
fn test_ids_drift_every_control_unique_per_window() -> Result<(), String> {
    // Arrange
    let block = machine_block(GUI_WINDOWS_DOC)?;
    let windows = block
        .get("windows")
        .and_then(Value::as_object)
        .ok_or("the document has no windows map")?;

    // Act
    let repeated: Vec<&str> = windows
        .iter()
        .filter(|(_, controls)| {
            let list = controls.as_array().map(Vec::as_slice).unwrap_or_default();
            let unique: BTreeSet<&str> = list.iter().filter_map(Value::as_str).collect();
            unique.len() != list.len()
        })
        .map(|(window, _)| window.as_str())
        .collect();

    // Assert
    assert_eq!(repeated, Vec::<&str>::new());
    assert_eq!(windows.len(), 30);
    Ok(())
}

#[test]
fn test_ids_drift_every_id_is_referenced() -> Result<(), String> {
    // Arrange
    let block = machine_block(GUI_WINDOWS_DOC)?;
    let doc_windows: BTreeSet<&str> = block
        .get("windows")
        .and_then(Value::as_object)
        .ok_or("the document has no windows map")?
        .keys()
        .map(String::as_str)
        .collect();
    let registry_windows: BTreeSet<&str> =
        ids::WINDOWS.iter().map(|window| window.window).collect();
    let doc_actions: Vec<&str> = block
        .get("actions")
        .and_then(Value::as_array)
        .ok_or("the document has no actions array")?
        .iter()
        .filter_map(|row| row.get("id").and_then(Value::as_str))
        .collect();
    let unique_actions: BTreeSet<&str> = doc_actions.iter().copied().collect();
    let registry_actions: BTreeSet<&str> = ACTIONS.iter().map(|row| row.id.as_str()).collect();

    // Act and assert
    assert_eq!(doc_windows, registry_windows);
    assert_eq!(unique_actions, registry_actions);
    assert_eq!(unique_actions.len(), doc_actions.len());
    Ok(())
}

#[test]
fn test_the_section_15_map_has_45_rows() -> Result<(), String> {
    // The copied rows sit between the §15 heading and the accelerator heading, and every copied
    // row starts with a bold menu item (`| **`). No other table row in that span starts with `| **`.
    // Arrange
    let map = section(GUI_WINDOWS_DOC, "## 4. ", "## 5. ")?;

    // Act
    let rows = map.lines().filter(|line| line.starts_with("| **")).count();

    // Assert
    assert_eq!(rows, 45);
    Ok(())
}

#[test]
fn test_machine_block_without_a_json_fence_errors() {
    // Act
    let result = machine_block("# no block here\n");

    // Assert
    assert_eq!(
        result,
        Err("the document must hold exactly one ```json fence, found 0".to_owned())
    );
}

#[test]
fn test_machine_block_with_two_json_fences_errors() {
    // Arrange
    let doc = "```json\n{}\n```\n\n```json\n{}\n```\n";

    // Act
    let result = machine_block(doc);

    // Assert
    assert_eq!(
        result,
        Err("the document must hold exactly one ```json fence, found 2".to_owned())
    );
}

#[test]
fn test_machine_block_with_invalid_json_errors() {
    // Arrange
    let doc = "```json\nnot json\n```\n";

    // Act
    let result = machine_block(doc);

    // Assert
    assert!(matches!(
        result,
        Err(message) if message.starts_with("the machine block is not valid JSON")
    ));
}

#[test]
fn test_section_without_the_start_heading_errors() {
    // Act
    let result = section("body\n", "## 4. ", "## 5. ");

    // Assert
    assert_eq!(
        result,
        Err("the document has no \"## 4. \" heading".to_owned())
    );
}

#[test]
fn test_section_without_the_end_heading_errors() {
    // Act
    let result = section("## 4. \nbody\n", "## 4. ", "## 5. ");

    // Assert
    assert_eq!(
        result,
        Err("the document has no \"## 5. \" heading after \"## 4. \"".to_owned())
    );
}

#[test]
fn test_object_matches_reports_the_first_difference() {
    // Arrange
    let doc = json!({ "a": 1, "b": 2 });
    let registry = json!({ "a": 1, "b": 3 });
    let shorter = json!({ "a": 1, "c": 2 });

    // Act
    let result = object_matches("windows", Some(&doc), Some(&registry));
    let missing = object_matches("windows", Some(&shorter), Some(&registry));

    // Assert
    assert_eq!(result, Err("windows: key b expected 3, found 2".to_owned()));
    assert_eq!(
        missing,
        Err("windows: key b expected 3, found missing".to_owned())
    );
}

#[test]
fn test_object_matches_rejects_a_non_object_on_either_side() {
    // Arrange
    let object = json!({});
    let number = json!(1);

    // Act
    let document = object_matches("windows", Some(&json!([])), Some(&object));
    let registry = object_matches("windows", Some(&object), Some(&number));

    // Assert
    assert_eq!(
        document,
        Err("windows: the document entry is not an object".to_owned())
    );
    assert_eq!(
        registry,
        Err("windows: the registry entry is not an object".to_owned())
    );
}

#[test]
fn test_array_matches_reports_the_first_difference() {
    // Arrange
    let doc = json!([1, 2, 3]);
    let registry = json!([1, 9, 3]);

    // Act
    let result = array_matches("actions", Some(&doc), Some(&registry));

    // Assert
    assert_eq!(
        result,
        Err("actions: entry 1 expected 9, found 2".to_owned())
    );
}

#[test]
fn test_array_matches_reports_a_length_difference() {
    // Arrange
    let doc = json!([1]);
    let registry = json!([1, 2]);

    // Act
    let result = array_matches("fields", Some(&doc), Some(&registry));

    // Assert
    assert_eq!(
        result,
        Err("fields: entry count 1 does not match the registry count 2".to_owned())
    );
}

#[test]
fn test_array_matches_rejects_a_non_array_on_either_side() {
    // Arrange
    let array = json!([]);
    let number = json!(1);

    // Act
    let document = array_matches("fields", Some(&json!({})), Some(&array));
    let registry = array_matches("fields", Some(&array), Some(&number));

    // Assert
    assert_eq!(
        document,
        Err("fields: the document entry is not an array".to_owned())
    );
    assert_eq!(
        registry,
        Err("fields: the registry entry is not an array".to_owned())
    );
}
