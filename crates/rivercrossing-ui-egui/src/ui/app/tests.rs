// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use craft::egui::{
    Context, Event, Modifiers, PointerButton, Pos2, RawInput, Vec2, ViewportCommand, ViewportId,
};
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::runtime::access::Role;
use craft::ui::shell::app::ShellApp;
use craft::ui::shell::embed::CloseDecision;
use craft::ui::testing::Fixture;
use craft::ui::testing::access::{AccessTree, violations};

use super::RiverCrossingApp;

#[test]
fn app_ui_renders_one_frame_without_panicking() {
    // Arrange
    let mut app = RiverCrossingApp::new(None, None);
    let ctx = Context::default();

    // Act
    let output = ctx.run_ui(RawInput::default(), |ui| app.ui(ui));
    let rendered_root = output.viewport_output.contains_key(&ViewportId::ROOT);

    // Assert
    assert!(rendered_root);
    output.drop_without_applying_deltas();
}

#[test]
fn app_close_request_is_close() {
    // Arrange
    let mut app = RiverCrossingApp::new(None, None);

    // Act
    let decision = app.close_requested();

    // Assert
    assert_eq!(decision, CloseDecision::Close);
}

#[test]
fn app_logic_emits_viewport_close_when_the_quit_deadline_passes() {
    // Arrange
    let mut app = RiverCrossingApp::new(Some(Duration::ZERO), None);
    let ctx = Context::default();

    // Act
    let output = ctx.run_logic(&RawInput::default(), |ctx| app.logic(ctx));

    // Assert
    let commands = output
        .viewport_commands
        .get(&ViewportId::ROOT)
        .expect("the app asked the root viewport to close");
    assert!(commands.contains(&ViewportCommand::Close));
}

#[test]
fn app_logic_emits_no_close_before_the_deadline() {
    // Arrange
    let mut app = RiverCrossingApp::new(None, None);
    let ctx = Context::default();

    // Act
    let output = ctx.run_logic(&RawInput::default(), |ctx| app.logic(ctx));

    // Assert
    let closes = output
        .viewport_commands
        .get(&ViewportId::ROOT)
        .is_some_and(|commands| commands.contains(&ViewportCommand::Close));
    assert!(
        !closes,
        "the app sent ViewportCommand::Close without a deadline"
    );
}

#[test]
fn app_logic_emits_no_close_when_the_quit_deadline_is_in_the_future() {
    // Arrange
    let mut app = RiverCrossingApp::new(Some(Duration::from_secs(3600)), None);
    let ctx = Context::default();

    // Act
    let output = ctx.run_logic(&RawInput::default(), |ctx| app.logic(ctx));

    // Assert
    let closes = output
        .viewport_commands
        .get(&ViewportId::ROOT)
        .is_some_and(|commands| commands.contains(&ViewportCommand::Close));
    assert!(
        !closes,
        "the app sent ViewportCommand::Close before its deadline"
    );
}

#[test]
fn app_new_tolerates_a_duration_too_large_for_the_clock() {
    // Arrange
    let mut app = RiverCrossingApp::new(Some(Duration::MAX), None);
    let ctx = Context::default();

    // Act
    let output = ctx.run_logic(&RawInput::default(), |ctx| app.logic(ctx));

    // Assert
    let closes = output
        .viewport_commands
        .get(&ViewportId::ROOT)
        .is_some_and(|commands| commands.contains(&ViewportCommand::Close));
    assert!(!closes, "an overflowing deadline must not close the window");
}

/// A fixture over a fresh app, sized to the shell window and carrying the app fonts.
fn app_fixture() -> Fixture {
    let app = Rc::new(RefCell::new(RiverCrossingApp::new(None, None)));
    let mut fixture = Fixture::new(move |ui| app.borrow_mut().ui(ui));
    fixture.room(Vec2::new(1100.0, 780.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture
}

/// A primary-button event at `pos`.
fn pointer_button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Whether any node's name or value contains `needle`.
fn tree_mentions(tree: &AccessTree, needle: &str) -> bool {
    tree.keys().into_iter().any(|key| {
        tree.name_of(key).is_some_and(|name| name.contains(needle))
            || tree
                .value_of(key)
                .is_some_and(|value| value.contains(needle))
    })
}

#[test]
fn test_fixture_probe_button_is_reachable_by_label() {
    // Arrange
    let mut fixture = app_fixture();
    fixture.idle(2);

    // Act
    let target = fixture
        .by_label("Probe Echo")
        .expect("the probe button carries its name");

    // Assert
    assert_eq!(fixture.tree().role_of(target.node), Some(Role::BUTTON));
    assert!(
        target.rect.width() > 0.0 && target.rect.height() > 0.0,
        "the probe button has bounds: {:?}",
        target.rect
    );
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
}

#[test]
fn test_fixture_probe_input_is_reachable_by_label() {
    // Arrange
    let mut fixture = app_fixture();
    fixture.idle(2);

    // Act
    let target = fixture
        .by_label("Probe Input")
        .expect("the probe field carries its explicit accessible name");

    // Assert
    assert_eq!(fixture.tree().role_of(target.node), Some(Role::TEXT_INPUT));
    assert!(
        target.rect.width() > 0.0 && target.rect.height() > 0.0,
        "the probe field has bounds: {:?}",
        target.rect
    );
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
}

#[test]
fn test_fixture_has_no_accessibility_violations() {
    // Arrange
    let mut fixture = app_fixture();

    // Act
    fixture.idle(1);
    let first = violations(fixture.ctx(), fixture.tree());
    fixture.idle(1);
    let second = violations(fixture.ctx(), fixture.tree());

    // Assert
    assert_eq!(first, Vec::<String>::new());
    assert_eq!(second, Vec::<String>::new());
}

#[test]
fn test_fixture_probe_click_echoes_into_the_tree() {
    // Arrange
    let mut fixture = app_fixture();
    fixture.idle(2);
    let center = fixture
        .by_label("Probe Echo")
        .expect("the probe button carries its name")
        .center();

    // Act
    fixture.frame(vec![Event::PointerMoved(center)]);
    fixture.frame(vec![pointer_button(center, true)]);
    fixture.frame(vec![pointer_button(center, false)]);
    fixture.idle(1);

    // Assert
    assert!(
        tree_mentions(fixture.tree(), "clicked 1"),
        "the probe label shows the click"
    );
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
}
