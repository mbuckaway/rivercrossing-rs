// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the self-test report's bounded, scrolling panel.

use std::cell::RefCell;
use std::rc::Rc;

use craft::egui::Vec2;
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::testing::Fixture;

use super::{REPORT_HEIGHT, body};
use crate::ui::state::AppState;

/// Return a state holding *lines* self-test lines.
fn report_state(lines: usize) -> Rc<RefCell<AppState>> {
    let state = Rc::new(RefCell::new(AppState::blank()));
    state.borrow_mut().selftest_lines = (0..lines)
        .map(|line| format!("check {line:02} ................ PASS"))
        .collect();
    state
}

/// Return a fixture showing the self-test body over *state*.
fn report_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        body(ui, &mut shown.borrow_mut());
    });
    fixture.room(Vec2::new(640.0, 760.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(3);
    fixture
}

#[test]
fn test_selftest_body_given_a_long_report_clips_it_to_the_panel() {
    // Arrange
    let state = report_state(60);

    // Act
    let fixture = report_fixture(&state);
    let widest = fixture
        .texts()
        .iter()
        .filter(|run| run.text.starts_with("check "))
        .map(|run| run.clip.height())
        .fold(0.0_f32, f32::max);

    // Assert: every line is painted under the panel's own scroll viewport.
    assert!(widest > 0.0, "the report shows");
    assert!(
        widest <= REPORT_HEIGHT + 1.0,
        "the lines clip to the panel: {widest}"
    );
}

#[test]
fn test_selftest_body_given_a_long_report_scrolls_past_the_panel() {
    // Arrange
    let state = report_state(60);

    // Act
    let fixture = report_fixture(&state);
    let beyond = fixture
        .texts()
        .iter()
        .any(|run| run.text.starts_with("check ") && run.rect.max.y > run.clip.max.y + 1.0);

    // Assert: the panel is shorter than its report, so the rest scrolls into view.
    assert!(beyond, "the report runs past the panel");
}
