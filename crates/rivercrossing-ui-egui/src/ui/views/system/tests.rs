// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the About box's stat rows and the registry-generated shortcut table.

use craft::egui::{Rect, Vec2};
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::testing::Fixture;

use super::{AUTHOR, COPYRIGHT, GORBA_URL, LICENSE, VERSION, about, shortcut_table};
use crate::ui::action::spec::ACTIONS;

/// Return the rect of the first painted run whose text is exactly *text*.
fn text_rect(fixture: &Fixture, text: &str) -> Rect {
    fixture
        .texts()
        .iter()
        .find(|run| run.text == text)
        .map(|run| run.rect)
        .expect("the run shows")
}

/// Return the shortcut rows the registry documents.
fn documented() -> Vec<(&'static str, String)> {
    ACTIONS
        .iter()
        .filter_map(|row| row.shortcut.map(|shortcut| (row.label, shortcut.text())))
        .collect()
}

#[test]
fn test_shortcut_table_given_the_registry_lists_only_the_documented_shortcuts() {
    // Arrange
    let documented = documented();

    // Act
    let table = shortcut_table();

    // Assert: one row per documented shortcut, and no row without one.
    assert_eq!(table.rows.len(), documented.len());
    let labels: Vec<&str> = table.rows.iter().map(|row| row.label.as_str()).collect();
    let expected: Vec<&str> = documented.iter().map(|(label, _)| *label).collect();
    assert_eq!(labels, expected);
}

#[test]
fn test_shortcut_table_given_the_registry_keeps_its_two_columns() {
    // Arrange

    // Act
    let table = shortcut_table();

    // Assert
    let headers: Vec<&str> = table
        .columns
        .iter()
        .map(|column| column.header.as_str())
        .collect();
    assert_eq!(headers, ["Command", "Shortcut"]);
}

#[test]
fn test_shortcut_table_given_the_registry_drops_the_rows_without_a_shortcut() {
    // Arrange

    // Act
    let table = shortcut_table();
    let has_library = table.rows.iter().any(|row| row.label == "Ride Library");

    // Assert: the Ride Library action has no shortcut row.
    assert!(!has_library, "a row without a shortcut never shows");
}

/// Assert that *value* sits under *caption*, on the caption's edge.
fn assert_stat_row(fixture: &Fixture, caption: &str, value: &str) {
    let label = text_rect(fixture, caption);
    let shown = text_rect(fixture, value);
    assert!(
        shown.min.y >= label.max.y - 0.5,
        "{caption} puts its value below the caption: {label:?} {shown:?}"
    );
    assert!(
        (shown.min.x - label.min.x).abs() < 0.5,
        "{caption} keeps its value on the caption's edge: {label:?} {shown:?}"
    );
}

#[test]
fn test_about_given_its_metadata_puts_each_value_under_its_caption() {
    // Arrange
    let mut fixture = Fixture::new(about);
    fixture.room(Vec2::new(500.0, 420.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(2);

    // Act and assert: a stat row per value, with the value below its caption.
    assert_stat_row(&fixture, "Version", VERSION);
    assert_stat_row(&fixture, "Author", AUTHOR);
    assert_stat_row(&fixture, "Copyright", COPYRIGHT);
    assert_stat_row(&fixture, "License", LICENSE);
    assert_stat_row(&fixture, "URL", GORBA_URL);
}
