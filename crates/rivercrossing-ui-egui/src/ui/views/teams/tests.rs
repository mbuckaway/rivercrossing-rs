// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the teams editor's two-pane layout, its one-row actions and the Add/Edit team
//! form's notes and logo card rows.

use std::cell::RefCell;
use std::rc::Rc;

use craft::egui::{Rect, Vec2};
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::runtime::access::Role;
use craft::ui::testing::Fixture;
use rivercrossing_cards::{JokersMode, Shoe};
use rivercrossing_ride::{
    EntryId, EntryMode, PlateModel, RideConfig, RideConfigParams, RideEngine, SystemClock,
};
use rivercrossing_roster::roster::Roster;

use super::{RELAY_HELP, editor_body, form_body};
use crate::ui::presenters::teams::TeamRow;
use crate::ui::state::{AppState, LoadedRide, TeamEditorRow};

/// Return one editor row.
fn row(name: &str, plate: &str, members: usize) -> TeamEditorRow {
    TeamEditorRow {
        row: TeamRow {
            name: name.to_owned(),
            plate: plate.to_owned(),
            members,
        },
        entry: EntryId(name.to_owned()),
    }
}

/// Return a state whose teams editor holds the mockup's two teams and a selection.
fn editor_state() -> Rc<RefCell<AppState>> {
    let state = Rc::new(RefCell::new(AppState::blank()));
    let opened = state.borrow_mut().dialogs.open("team_editor_dlg", None);
    // The frozen dialog accepts its own id.
    assert_eq!(opened, Ok(()));
    {
        let mut app = state.borrow_mut();
        app.team_editor.rows = vec![row("River Rats", "A1", 4), row("Mud Dawgs", "A2", 4)];
        app.team_editor.selected = Some(EntryId("River Rats".to_owned()));
    }
    state
}

/// Return a loaded ride over `plate_model`, from the in-memory engine.
fn ride(plate_model: PlateModel) -> LoadedRide {
    let config = RideConfig::new(RideConfigParams {
        name: "GORBA EPIC 2026".to_owned(),
        event_date: time::Date::from_calendar_date(2026, time::Month::June, 13)
            .expect("the mockup date"),
        venue: "GORBA Park".to_owned(),
        lap_km: 7.5,
        organizer: "GORBA".to_owned(),
        scorer: "M. Buckaway".to_owned(),
        entry_mode: EntryMode::Mixed,
        plate_model,
        min_lap_s: 1200,
        planned_duration_s: 14_400,
        ..RideConfigParams::default()
    })
    .expect("the fixture config");
    let roster =
        Roster::new(EntryMode::Mixed, 4, plate_model, Some(7)).expect("the fixture roster");
    let shoe = Shoe::new(8, 1, 7, JokersMode::Total).expect("the fixture shoe");
    let engine = RideEngine::new(config.clone(), shoe, SystemClock, roster.clone());
    LoadedRide {
        ride_id: 1,
        config,
        roster,
        engine,
    }
}

/// Return a state whose Add team dialog is open over a ride with `plate_model`.
fn form_state(plate_model: PlateModel) -> Rc<RefCell<AppState>> {
    let state = Rc::new(RefCell::new(AppState::blank()));
    {
        let mut app = state.borrow_mut();
        app.ride = Some(ride(plate_model));
        assert_eq!(app.dialogs.open("add_team_dlg", None), Ok(()));
        for (field, value) in [
            ("name_input", "River Rats"),
            ("relay_plate_input", "A1"),
            ("notes_input", "The river's own team"),
        ] {
            let _ = app.dialogs.set_field(field, value);
        }
    }
    state
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

/// Return a fixture showing the teams editor body over *state*.
fn editor_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    body_fixture(state, Vec2::new(1100.0, 900.0), editor_body)
}

/// Return a fixture showing the Add team form over *state*.
fn form_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    body_fixture(state, Vec2::new(760.0, 720.0), form_body)
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
fn test_editor_puts_the_record_form_right_of_the_team_list() {
    // Arrange
    let state = editor_state();

    // Act
    let fixture = editor_fixture(&state);
    let header = text_rect(&fixture, "Riders");
    let record = text_rect(&fixture, "Name");

    // Assert: the list ends before the record pane starts.
    assert!(
        record.min.x > header.max.x,
        "the record pane sits right of the list: {record:?} {header:?}"
    );
}

#[test]
fn test_editor_lists_only_the_team_and_riders_columns() {
    // Arrange
    let state = editor_state();

    // Act
    let fixture = editor_fixture(&state);
    let riders = text_rect(&fixture, "Riders");
    let record = text_rect(&fixture, "Name");
    let mut labels: Vec<(f32, &str)> = fixture
        .texts()
        .iter()
        .filter(|run| same_row(run.rect, riders) && run.rect.max.x < record.min.x)
        .map(|run| (run.rect.min.x, run.text.as_str()))
        .collect();
    labels.sort_by(|left, right| left.0.total_cmp(&right.0));

    // Assert: the mockup's two columns, and the old Plate column gone.
    let shown: Vec<&str> = labels.into_iter().map(|(_, text)| text).collect();
    assert_eq!(shown, ["Team", "Riders"]);
    assert!(!paints(&fixture, "Plate"), "the Plate column is gone");
}

#[test]
fn test_editor_puts_the_actions_in_one_row_above_the_list() {
    // Arrange
    let state = editor_state();

    // Act
    let fixture = editor_fixture(&state);
    let filter = fixture
        .by_label("Single-member teams only")
        .expect("the filter")
        .rect;
    let add = fixture.by_label("Add team…").expect("the Add button").rect;
    let edit = fixture
        .by_label("Edit Team…")
        .expect("the Edit button")
        .rect;
    let remove = fixture.by_label("Remove").expect("the Remove button").rect;
    let header = text_rect(&fixture, "Team");

    // Assert: the mockup's one row, the filter at the start and the buttons at the end.
    assert!(same_row(filter, add), "the filter shares the buttons' row");
    assert!(same_row(add, edit), "Add and Edit share one row");
    assert!(same_row(edit, remove), "Edit and Remove share one row");
    assert!(filter.min.x < add.min.x, "the filter leads the row");
    assert!(add.max.x <= edit.min.x, "Add leads Edit");
    assert!(edit.max.x <= remove.min.x, "Edit leads Remove");
    assert!(
        remove.max.y <= header.min.y,
        "the row stands above the table: {remove:?} {header:?}"
    );
}

// -------------------------------------------------------------------- the form

#[test]
fn test_form_notes_field_is_two_lines() {
    // Arrange
    let state = form_state(PlateModel::RiderPooled);

    // Act
    let fixture = form_fixture(&state);
    let notes = fixture.by_role(Role::MULTILINE_TEXT_INPUT);

    // Assert: the one multi-line field is the notes textarea.
    assert_eq!(notes.len(), 1, "only Notes is multi-line");
    let named = notes
        .first()
        .and_then(|target| fixture.tree().name_of(target.node));
    assert_eq!(named, Some("Notes"));
}

#[test]
fn test_form_shows_the_logo_card_row_with_browse() {
    // Arrange
    let state = form_state(PlateModel::RiderPooled);

    // Act
    let fixture = form_fixture(&state);
    let browse = fixture.by_label("Browse…");

    // Assert: the mockup's "Logo card" row, not the raw NO LOGO word.
    assert!(paints(&fixture, "Logo card"), "the row is titled");
    assert!(browse.is_some(), "the row carries Browse…");
    assert!(!paints(&fixture, "NO LOGO"), "the raw status word is gone");
}

#[test]
fn test_form_shows_the_relay_help_on_a_relay_ride() {
    // Arrange
    let state = form_state(PlateModel::TeamRelay);

    // Act
    let fixture = form_fixture(&state);
    let plate = text_rect(&fixture, "Relay plate");
    let help = text_rect(&fixture, RELAY_HELP);

    // Assert: the mockup's help line under the field.
    assert!(
        help.min.y >= plate.max.y,
        "the help sits under Relay plate: {help:?} {plate:?}"
    );
}

#[test]
fn test_form_hides_the_relay_row_on_a_pooled_ride() {
    // Arrange
    let state = form_state(PlateModel::RiderPooled);

    // Act
    let fixture = form_fixture(&state);

    // Assert
    assert!(!paints(&fixture, "Relay plate"), "the row is hidden");
    assert!(!paints(&fixture, RELAY_HELP), "the help is hidden");
}

#[test]
fn test_editor_clicking_the_filter_stores_it() {
    // Arrange
    let state = editor_state();
    let mut fixture = editor_fixture(&state);
    let filter = fixture
        .by_label("Single-member teams only")
        .expect("the filter");

    // Act
    fixture.click(&filter);
    fixture.idle(1);

    // Assert: the frozen field carries the filter to the presenter.
    assert_eq!(
        state.borrow().dialogs.field("single_member_only_chk"),
        Some("true")
    );
}
