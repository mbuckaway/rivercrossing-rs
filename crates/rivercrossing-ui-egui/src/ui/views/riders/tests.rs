// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the rider editor's two-pane layout and the Add/Edit rider grid.

use std::cell::RefCell;
use std::rc::Rc;

use craft::egui::{Rect, Vec2};
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::runtime::access::Role;
use craft::ui::testing::Fixture;
use rivercrossing_ride::EntryId;

use super::{DELETE_NOTICE, PLATE_NOTICE, editor_body, form_body, selection_at};
use crate::ui::presenters::riders::RiderRow;
use crate::ui::state::{AppState, RiderEditorRow, RiderSelection};

/// Return one editor row over entry `e1`.
fn row(plate: &str, name: &str, team: &str, rider_index: usize) -> RiderEditorRow {
    RiderEditorRow {
        row: RiderRow {
            plate: plate.to_owned(),
            name: name.to_owned(),
            team: team.to_owned(),
        },
        entry: EntryId("e1".to_owned()),
        rider_index,
    }
}

/// Return a state whose rider editor holds the mockup's two riders and a selection.
fn editor_state() -> Rc<RefCell<AppState>> {
    let state = Rc::new(RefCell::new(AppState::blank()));
    let opened = state.borrow_mut().dialogs.open("rider_editor_dlg", None);
    // The frozen dialog accepts its own id.
    assert_eq!(opened, Ok(()));
    {
        let mut app = state.borrow_mut();
        app.rider_editor.rows = vec![
            row("123", "Sam Ellis", "River Rats", 0),
            row("077", "Ana Ruiz", "River Rats", 1),
        ];
        app.rider_editor.selected = Some(RiderSelection {
            entry: EntryId("e1".to_owned()),
            rider_index: 0,
        });
    }
    state
}

/// Return a state whose Add rider dialog is open with the mockup's form values.
fn add_state() -> Rc<RefCell<AppState>> {
    let state = Rc::new(RefCell::new(AppState::blank()));
    {
        let mut app = state.borrow_mut();
        assert_eq!(app.dialogs.open("add_rider_dlg", None), Ok(()));
        for (field, value) in [
            ("plate_input", "123"),
            ("first_name_input", "Sam"),
            ("last_name_input", "Ellis"),
            ("team_choice", "solo"),
            ("sex_choice", ""),
        ] {
            let _ = app.dialogs.set_field(field, value);
        }
    }
    state
}

/// Return a fixture showing the rider editor body over *state* in a wide room.
fn editor_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    body_fixture(state, Vec2::new(1100.0, 900.0), editor_body)
}

/// Return a fixture showing the Add rider form over *state*.
fn form_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    body_fixture(state, Vec2::new(760.0, 720.0), form_body)
}

/// Return a fixture showing `body` over *state*.
fn body_fixture(
    state: &Rc<RefCell<AppState>>,
    room: Vec2,
    body: fn(&mut craft::egui::Ui, &mut AppState),
) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        body(ui, &mut shown.borrow_mut());
    });
    fixture.room(room);
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

/// Return whether the latest frame painted *needle* anywhere.
fn paints(fixture: &Fixture, needle: &str) -> bool {
    fixture.texts().iter().any(|run| run.text.contains(needle))
}

/// Return whether two rects share a vertical band.
fn same_row(left: Rect, right: Rect) -> bool {
    left.min.y < right.max.y && right.min.y < left.max.y
}

// ------------------------------------------------------------------ the split

#[test]
fn test_editor_puts_the_record_form_right_of_the_roster() {
    // Arrange
    let state = editor_state();

    // Act
    let fixture = editor_fixture(&state);
    let header = text_rect(&fixture, "Sex");
    let record = text_rect(&fixture, "First name");

    // Assert: the last table column ends before the record pane starts.
    assert!(
        record.min.x > header.max.x,
        "the record pane sits right of the list: {record:?} {header:?}"
    );
}

#[test]
fn test_editor_reserves_the_mockups_list_height() {
    // Arrange
    let state = editor_state();

    // Act
    let fixture = editor_fixture(&state);
    let header = text_rect(&fixture, "Name");
    let notice = text_rect(&fixture, DELETE_NOTICE);

    // Assert: `.split` reserves 300 pt, so the notice lands past it.
    assert!(
        notice.min.y - header.min.y >= 280.0,
        "the list keeps its 300 pt floor: {notice:?} {header:?}"
    );
}

#[test]
fn test_editor_puts_add_and_delete_at_the_list_panes_bottom() {
    // Arrange
    let state = editor_state();

    // Act
    let fixture = editor_fixture(&state);
    let add = fixture.by_label("Add rider…").expect("the Add button").rect;
    let delete = fixture.by_label("Delete").expect("the Delete button").rect;
    let record = text_rect(&fixture, "First name");
    let notice = text_rect(&fixture, DELETE_NOTICE);

    // Assert: both sit at the list pane's foot, left of the record pane.
    assert!(add.max.x < record.min.x, "Add stays in the list: {add:?}");
    assert!(
        delete.max.x < record.min.x,
        "Delete stays in the list: {delete:?}"
    );
    assert!(notice.min.y - add.max.y < 40.0, "Add sits at the bottom");
    assert!(
        notice.min.y - delete.max.y < 40.0,
        "Delete sits at the bottom"
    );
}

#[test]
fn test_editor_puts_edit_at_the_form_panes_bottom() {
    // Arrange
    let state = editor_state();

    // Act
    let fixture = editor_fixture(&state);
    let edit = fixture
        .by_label("Edit Rider…")
        .expect("the Edit button")
        .rect;
    let record = text_rect(&fixture, "First name");
    let notice = text_rect(&fixture, DELETE_NOTICE);

    // Assert: the edit acts on the record above it, at the form pane's foot.
    assert!(
        edit.min.x >= record.min.x,
        "Edit stays in the record pane: {edit:?} {record:?}"
    );
    assert!(notice.min.y - edit.max.y < 40.0, "Edit sits at the bottom");
}

#[test]
fn test_editor_lists_the_sex_column_after_name() {
    // Arrange
    let state = editor_state();

    // Act
    let fixture = editor_fixture(&state);
    let name = text_rect(&fixture, "Name");
    let sex = text_rect(&fixture, "Sex");

    // Assert
    assert!(same_row(name, sex), "Sex shares the header row: {name:?}");
    assert!(sex.min.x > name.min.x, "Sex follows Name: {sex:?}");
}

// ----------------------------------------------------------------- the notices

#[test]
fn test_editor_shows_the_delete_notice() {
    // Arrange
    let state = editor_state();

    // Act
    let fixture = editor_fixture(&state);

    // Assert: the mockup's exact line.
    assert!(
        paints(&fixture, DELETE_NOTICE),
        "the delete rule shows: {DELETE_NOTICE}"
    );
}

#[test]
fn test_editor_search_has_the_mockups_placeholder() {
    // Arrange
    let state = editor_state();

    // Act
    let fixture = editor_fixture(&state);

    // Assert
    assert!(
        paints(&fixture, "Search plate, name or team"),
        "the search field carries the mockup's prompt"
    );
}

// -------------------------------------------------------------------- the form

#[test]
fn test_form_puts_plate_beside_first_name() {
    // Arrange
    let state = add_state();

    // Act
    let fixture = form_fixture(&state);
    let plate = text_rect(&fixture, "Plate");
    let first = text_rect(&fixture, "First name");

    // Assert: the mockup's `.grid2` first row, split across the two columns.
    assert!(same_row(plate, first), "Plate shares First name's row");
    assert!(first.min.x > plate.max.x, "First name takes the right cell");
}

#[test]
fn test_form_puts_last_name_beside_team_under_them() {
    // Arrange
    let state = add_state();

    // Act
    let fixture = form_fixture(&state);
    let plate = text_rect(&fixture, "Plate");
    let last = text_rect(&fixture, "Last name");
    let team = text_rect(&fixture, "Team");

    // Assert: the grid's second row, under the first.
    assert!(same_row(last, team), "Last name shares Team's row");
    assert!(
        last.min.y >= plate.max.y,
        "the second row sits under the first"
    );
}

#[test]
fn test_form_puts_sex_under_last_name() {
    // Arrange
    let state = add_state();

    // Act
    let fixture = form_fixture(&state);
    let last = text_rect(&fixture, "Last name");
    let sex = text_rect(&fixture, "Sex");

    // Assert: the third row's left cell.
    assert!(sex.min.y >= last.max.y, "Sex sits under Last name");
    assert!(
        (sex.min.x - last.min.x).abs() < 2.0,
        "Sex keeps the left column: {sex:?} {last:?}"
    );
}

#[test]
fn test_form_select_fills_its_cell() {
    // Arrange
    let state = add_state();

    // Act
    let fixture = form_fixture(&state);
    let plate = fixture
        .by_role(Role::TEXT_INPUT)
        .first()
        .map(|target| target.rect)
        .expect("the Plate field");
    let sex = fixture
        .by_role(Role::COMBO_BOX)
        .last()
        .map(|target| target.rect)
        .expect("the Sex select");

    // Assert: the select and the field take the same cell, not their text's width.
    assert!(
        (sex.width() - plate.width()).abs() < 8.0,
        "the select fills its cell: {sex:?} {plate:?}"
    );
    assert!(sex.width() > 150.0, "the cell is a column: {sex:?}");
}

#[test]
fn test_form_shows_the_plate_notice() {
    // Arrange
    let state = add_state();

    // Act
    let fixture = form_fixture(&state);

    // Assert: the mockup's exact line.
    assert!(
        paints(&fixture, PLATE_NOTICE),
        "the plate rule shows: {PLATE_NOTICE}"
    );
}

// -------------------------------------------------------------- the selection

#[test]
fn test_selection_at_given_no_selection_returns_false() {
    // Arrange: the editor holds a row but nothing is selected.
    let mut state = AppState::blank();
    state.rider_editor.rows = vec![row("123", "Sam Ellis", "River Rats", 0)];

    // Act
    let chosen = selection_at(&state, 0);

    // Assert
    assert!(!chosen);
}

#[test]
fn test_selection_at_given_the_selected_row_returns_true() {
    // Arrange
    let mut state = AppState::blank();
    state.rider_editor.rows = vec![row("123", "Sam Ellis", "River Rats", 0)];
    state.rider_editor.selected = Some(RiderSelection {
        entry: EntryId("e1".to_owned()),
        rider_index: 0,
    });

    // Act
    let chosen = selection_at(&state, 0);

    // Assert
    assert!(chosen);
}

#[test]
fn test_selection_at_given_another_row_returns_false() {
    // Arrange: the selection names the second row.
    let mut state = AppState::blank();
    state.rider_editor.rows = vec![
        row("123", "Sam Ellis", "River Rats", 0),
        row("077", "Ana Ruiz", "River Rats", 1),
    ];
    state.rider_editor.selected = Some(RiderSelection {
        entry: EntryId("e1".to_owned()),
        rider_index: 1,
    });

    // Act
    let chosen = selection_at(&state, 0);

    // Assert
    assert!(!chosen);
}
