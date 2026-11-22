// SPDX-License-Identifier: GPL-3.0-only

use std::time::Duration;

use craft::egui::{Context, RawInput, ViewportCommand, ViewportId};
use craft::ui::shell::app::ShellApp;
use craft::ui::shell::embed::CloseDecision;

use super::RiverCrossingApp;

#[test]
fn app_ui_renders_one_frame_without_panicking() {
    // Arrange
    let mut app = RiverCrossingApp::new(None);
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
    let mut app = RiverCrossingApp::new(None);

    // Act
    let decision = app.close_requested();

    // Assert
    assert_eq!(decision, CloseDecision::Close);
}

#[test]
fn app_logic_emits_viewport_close_when_the_quit_deadline_passes() {
    // Arrange
    let mut app = RiverCrossingApp::new(Some(Duration::ZERO));
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
    let mut app = RiverCrossingApp::new(None);
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
    let mut app = RiverCrossingApp::new(Some(Duration::from_secs(3600)));
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
    let mut app = RiverCrossingApp::new(Some(Duration::MAX));
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
