// SPDX-License-Identifier: GPL-3.0-only

//! Render the app's shell and compare its first settled frame with the committed golden.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use craft::egui::vec2;
use craft::host::target::ShellInfo;
use craft::ui::InstallSpec;
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::shell::app::ShellApp;
use craft::ui::shell::embed;
use craft::ui::testing::windows::NoWindows;
use egui_kittest::Harness;

use rivercrossing_ui_egui::ui::app::RiverCrossingApp;

#[test]
fn test_ui_shell_first_frame_matches_the_golden() {
    // Arrange
    let app = Rc::new(RefCell::new(RiverCrossingApp::new(None, None)));
    let ui_app = Rc::clone(&app);

    // Act
    let mut harness = Harness::builder()
        .with_size(vec2(1100.0, 780.0))
        .with_context_setup(|ctx| {
            let scripts = I18nScripts;
            let fonts = UiFonts::new(&scripts);
            let spec = InstallSpec::new(ShellInfo::default(), Arc::new(NoWindows));
            embed::install(ctx, spec, &fonts).expect("a fresh context takes the runtime");
        })
        .build_ui(move |ui| {
            embed::logic(ui.ctx(), || ui_app.borrow_mut().close_requested());
            ui_app.borrow_mut().ui(ui);
        });
    embed::before_input(&harness.ctx, |_| app.borrow_mut().frame_input());
    harness.run();

    // Assert
    harness.snapshot("shell");
}
