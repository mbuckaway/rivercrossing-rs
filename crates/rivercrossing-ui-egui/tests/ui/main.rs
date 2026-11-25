// SPDX-License-Identifier: GPL-3.0-only

//! Render the app's real screens and compare each settled frame with its committed golden.
//!
//! The goldens are written on the Linux golden host only (`UPDATE_SNAPSHOTS=force` with
//! `WGPU_BACKEND=vulkan`); `kittest.toml` holds the per-OS thresholds. Each app is headless over a
//! temporary settings file and database, so the run touches no user file and the rendering is
//! deterministic.

use std::cell::RefCell;
use std::error::Error;
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
use tempfile::TempDir;

use rivercrossing_ui_egui::ui::app::{AppPaths, RiverCrossingApp};

/// The headless app and the temporary directory its files live in.
struct GoldenFixture {
    /// The app under test.
    app: Rc<RefCell<RiverCrossingApp>>,
    /// The directory that holds the settings file and the database.
    _directory: TempDir,
}

/// A headless app over a fresh temporary directory, with light appearance and 100% zoom.
fn golden_app() -> Result<GoldenFixture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let paths = AppPaths::at(
        directory.path().join("settings.json"),
        directory.path().join("rides.db"),
        directory.path().to_path_buf(),
    );
    let mut settings = rivercrossing_store::default_settings();
    "light".clone_into(&mut settings.appearance);
    settings.zoom_percent = 100;
    rivercrossing_store::save_settings(&settings, &paths.settings)?;
    let app = RiverCrossingApp::headless(None, None, paths);
    Ok(GoldenFixture {
        app: Rc::new(RefCell::new(app)),
        _directory: directory,
    })
}

/// Build the settled harness over `app`.
fn build_harness(app: &Rc<RefCell<RiverCrossingApp>>) -> Result<Harness<'static>, Box<dyn Error>> {
    let failure: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let install_error = Rc::clone(&failure);
    let ui_app = Rc::clone(app);
    let mut harness = Harness::builder()
        .with_size(vec2(1100.0, 780.0))
        .with_context_setup(move |ctx| {
            let scripts = I18nScripts;
            let fonts = UiFonts::new(&scripts);
            let spec = InstallSpec::new(ShellInfo::default(), Arc::new(NoWindows));
            if let Err(error) = embed::install(ctx, spec, &fonts) {
                *install_error.borrow_mut() = Some(error.to_string());
            }
        })
        .build_ui(move |ui| {
            embed::logic(ui.ctx(), || ui_app.borrow_mut().close_requested());
            ui_app.borrow_mut().ui(ui);
        });
    if let Some(error) = failure.borrow_mut().take() {
        return Err(error.into());
    }
    embed::before_input(&harness.ctx, |_| app.borrow_mut().frame_input());
    harness.run();
    Ok(harness)
}

#[test]
fn test_ui_console_empty_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;

    // Act
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("console_empty");
    Ok(())
}

#[test]
fn test_ui_setup_open_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    {
        let mut app = fixture.app.borrow_mut();
        app.command("mi_new_ride")?;
        app.set_dialog_field("date_picker", "2026-11-25")?;
    }

    // Act
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("setup_open");
    Ok(())
}
