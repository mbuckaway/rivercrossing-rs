// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the `WordPress` publish form's fixed help line.

use std::cell::RefCell;
use std::rc::Rc;

use craft::egui::Vec2;
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::testing::Fixture;

use super::{HELP, form_body};
use crate::ui::state::AppState;

/// Return a fixture showing the publish form over *state*.
fn publish_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        form_body(ui, &mut shown.borrow_mut());
    });
    fixture.room(Vec2::new(520.0, 720.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(3);
    fixture
}

#[test]
fn test_publish_form_shows_the_help_line_under_its_selects() {
    // Arrange
    let state = Rc::new(RefCell::new(AppState::blank()));

    // Act
    let fixture = publish_fixture(&state);

    // Assert
    assert!(
        fixture.texts().iter().any(|run| run.text == HELP),
        "the mockup's help line sits under the kind and status selects"
    );
}
