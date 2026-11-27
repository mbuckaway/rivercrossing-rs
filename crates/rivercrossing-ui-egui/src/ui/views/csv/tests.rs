// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the CSV preview's conflict table and its warning.

use std::cell::RefCell;
use std::rc::Rc;

use craft::behavior::ColumnWidth;
use craft::egui::{Rect, Vec2};
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::testing::Fixture;

use super::{body, conflict_table};
use crate::ui::presenters::riders::{CsvConflict, CsvPreviewModel};
use crate::ui::state::AppState;

/// Return the mockup's two conflicts.
fn conflicts() -> Vec<CsvConflict> {
    vec![
        CsvConflict {
            row: 17,
            message: "Duplicate plate".to_owned(),
        },
        CsvConflict {
            row: 43,
            message: "Missing plate".to_owned(),
        },
    ]
}

/// Return a state whose CSV preview holds *conflicts*.
fn preview_state(conflicts: Vec<CsvConflict>) -> Rc<RefCell<AppState>> {
    let state = Rc::new(RefCell::new(AppState::blank()));
    state.borrow_mut().csv_preview.model = Some(CsvPreviewModel {
        summary: "174 riders parsed · 2 conflicts require a decision.".to_owned(),
        can_import: conflicts.is_empty(),
        conflicts,
    });
    state
}

/// Return a fixture showing the CSV preview body over *state*.
fn preview_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        body(ui, &mut shown.borrow_mut());
    });
    fixture.room(Vec2::new(760.0, 600.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(3);
    fixture
}

/// Return the rect of the first painted run whose text is exactly *text*.
fn text_rect(fixture: &Fixture, text: &str) -> Rect {
    fixture
        .texts()
        .iter()
        .find(|run| run.text == text)
        .map(|run| run.rect)
        .expect("the run shows")
}

#[test]
fn test_conflict_table_given_conflicts_shows_row_and_problem_columns() {
    // Arrange
    let conflicts = conflicts();

    // Act
    let data = conflict_table(&conflicts);

    // Assert
    let headers: Vec<&str> = data
        .columns
        .iter()
        .map(|column| column.header.as_str())
        .collect();
    assert_eq!(headers, ["Row", "Problem"]);
    assert_eq!(
        data.columns.first().map(|column| column.width),
        Some(ColumnWidth::Fixed(super::ROW_WIDTH))
    );
}

#[test]
fn test_conflict_table_given_a_problem_fills_the_remaining_width() {
    // Arrange
    let conflicts = conflicts();

    // Act
    let data = conflict_table(&conflicts);

    // Assert
    assert_eq!(
        data.columns.get(1).map(|column| column.width),
        Some(ColumnWidth::fill(1.0))
    );
}

#[test]
fn test_conflict_table_given_a_conflict_puts_the_row_beside_the_problem() {
    // Arrange
    let conflicts = conflicts();

    // Act
    let data = conflict_table(&conflicts);

    // Assert
    let first = data.rows.first().expect("the first conflict");
    assert_eq!(first.label, "Duplicate plate");
    assert_eq!(first.cells, vec![(super::ROW, "17".to_owned())]);
}

#[test]
fn test_csv_body_given_a_conflict_puts_the_warning_above_the_table() {
    // Arrange
    let state = preview_state(conflicts());

    // Act
    let fixture = preview_fixture(&state);
    let warning = text_rect(&fixture, "Import is disabled while conflicts remain.");
    let header = text_rect(&fixture, "Problem");

    // Assert
    assert!(
        warning.max.y <= header.min.y,
        "the warning sits above the table: {warning:?} {header:?}"
    );
}

#[test]
fn test_csv_body_given_a_conflict_does_not_elide_the_problem_header() {
    // Arrange
    let state = preview_state(conflicts());

    // Act
    let fixture = preview_fixture(&state);
    let header = fixture
        .texts()
        .iter()
        .find(|run| run.text == "Problem")
        .expect("the Problem header shows");

    // Assert
    assert!(!header.elided, "the Problem header shows in full");
}
