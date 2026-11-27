// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the ride library's status badge and its four columns.

use std::cell::RefCell;
use std::rc::Rc;

use craft::egui::Vec2;
use craft::ui::display::Badge;
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::testing::Fixture;

use super::body;
use crate::ui::presenters::library::RideRowModel;
use crate::ui::state::AppState;

/// Return the mockup's three rides.
fn rows() -> Vec<RideRowModel> {
    vec![
        RideRowModel {
            name: "GORBA EPIC 2026".to_owned(),
            date: "2026-06-13".to_owned(),
            status: "RUNNING".to_owned(),
            entries: 175,
        },
        RideRowModel {
            name: "Spring Classic".to_owned(),
            date: "2026-04-18".to_owned(),
            status: "FINISHED".to_owned(),
            entries: 142,
        },
        RideRowModel {
            name: "Test Ride #4".to_owned(),
            date: "2026-03-02".to_owned(),
            status: "DRAFT".to_owned(),
            entries: 0,
        },
    ]
}

/// Return a state whose library holds the mockup's rows.
fn library_state() -> Rc<RefCell<AppState>> {
    let state = Rc::new(RefCell::new(AppState::blank()));
    state.borrow_mut().library.rows = rows();
    state
}

/// Return a fixture showing the library body over *state*.
fn library_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        body(ui, &mut shown.borrow_mut());
    });
    fixture.room(Vec2::new(760.0, 500.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(3);
    fixture
}

#[test]
fn test_library_body_given_rides_draws_each_status_as_a_badge() {
    // Arrange
    let state = library_state();

    // Act
    let fixture = library_fixture(&state);
    let badges = fixture.styles_of(std::any::type_name::<Badge>()).len();

    // Assert: one badge per ride, not a plain text cell.
    assert_eq!(badges, 3, "each ride's status is a badge");
}

#[test]
fn test_library_body_given_a_running_ride_shows_the_status_word() {
    // Arrange
    let state = library_state();

    // Act
    let fixture = library_fixture(&state);
    let shown = fixture.texts().iter().any(|run| run.text == "RUNNING");

    // Assert: the badge says what its tone means.
    assert!(shown, "the RUNNING badge shows its word");
}

#[test]
fn test_library_body_given_rides_keeps_its_four_columns() {
    // Arrange
    let state = library_state();

    // Act
    let fixture = library_fixture(&state);
    let headers: Vec<&str> = fixture
        .texts()
        .iter()
        .map(|run| run.text.as_str())
        .filter(|text| ["Ride", "Date", "Status", "Entries"].contains(text))
        .collect();

    // Assert
    assert_eq!(headers, ["Ride", "Date", "Status", "Entries"]);
}

/// Return a fixture showing the Delete Ride body over *state*.
fn delete_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        super::delete_body(ui, &mut shown.borrow_mut());
    });
    fixture.room(Vec2::new(420.0, 240.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(3);
    fixture
}

#[test]
fn test_delete_body_caption_names_the_confirmation_the_contract_requires() {
    // Arrange
    let state = Rc::new(RefCell::new(AppState::blank()));

    // Act
    let fixture = delete_fixture(&state);
    let texts: Vec<&str> = fixture
        .texts()
        .iter()
        .map(|run| run.text.as_str())
        .collect();

    // Assert
    assert!(
        texts.contains(&"Type the ride name to confirm"),
        "the field caption carries the contract copy: {texts:?}"
    );
    assert!(
        !texts.contains(&"Ride name"),
        "the field caption no longer reads Ride name"
    );
}
