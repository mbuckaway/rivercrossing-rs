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

use rivercrossing_ride::{EntryMode, PlateModel, RideConfig, RideConfigParams};
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::{Entry, Rider};
use rivercrossing_store::Store;
use rivercrossing_ui_egui::ui::app::{AppPaths, RiverCrossingApp};

/// The headless app and the temporary directory its files live in.
struct GoldenFixture {
    /// The app under test.
    app: Rc<RefCell<RiverCrossingApp>>,
    /// The directory that holds the settings file and the database.
    directory: TempDir,
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
        directory,
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

/// Return one rider with the given name and own plate.
fn rider(first: &str, last: &str, plate: Option<&str>) -> Rider {
    Rider {
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        plate: plate.map(str::to_owned),
        ..Rider::default()
    }
}

/// Return the fixture ride's setup.
fn fixture_config() -> Result<RideConfig, String> {
    RideConfig::new(RideConfigParams {
        name: "GORBA EPIC 2026".to_owned(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        ..RideConfigParams::default()
    })
    .map_err(|error| error.to_string())
}

/// Return a roster with a solo pair and a two-rider team.
fn editor_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(7))
        .map_err(|error| error.to_string())?;
    roster.load_entries(vec![
        Entry::restored(
            "k-ada",
            "101",
            "Ada Lovelace",
            EntryType::Solo,
            vec![rider("Ada", "Lovelace", Some("101"))],
        ),
        Entry::restored(
            "k-grace",
            "102",
            "Grace Hopper",
            EntryType::Solo,
            vec![rider("Grace", "Hopper", Some("102"))],
        ),
        Entry::restored(
            "k-alpha",
            "A1",
            "Alpha",
            EntryType::Team,
            vec![
                rider("Alan", "Turing", Some("201")),
                rider("Barbara", "Liskov", Some("202")),
            ],
        ),
    ]);
    Ok(roster)
}

/// Return a roster that carries a team-of-one, a missing name and a missing number.
fn issues_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(7))
        .map_err(|error| error.to_string())?;
    roster
        .create_team_entry_of_one(TeamFields {
            display_name: "Lone Wolf",
            riders: vec![rider("Cy", "Three", Some("101"))],
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    roster.load_entries(vec![
        Entry::restored(
            "k-missing-name",
            "7",
            "",
            EntryType::Solo,
            vec![rider("", "", Some("7"))],
        ),
        Entry::restored(
            "k-missing-number",
            "9",
            "Grace Hopper",
            EntryType::Solo,
            vec![rider("Grace", "Hopper", None)],
        ),
    ]);
    Ok(roster)
}

/// Create a fixture ride over *roster* in *directory*, and return its database and id.
fn fixture_ride(
    directory: &TempDir,
    roster: &Roster,
) -> Result<(std::path::PathBuf, i64), Box<dyn Error>> {
    let path = directory.path().join("fixture.db");
    let mut store = Store::open(&path)?;
    let id = store.create_ride(&fixture_config()?, Some(7))?;
    store.save_roster(id, roster)?;
    Ok((path, id))
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

#[test]
fn test_ui_rider_editor_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.command("mi_rider_editor")?;
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("rider_editor");
    Ok(())
}

#[test]
fn test_ui_team_editor_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.command("mi_team_editor")?;
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("team_editor");
    Ok(())
}

#[test]
fn test_ui_csv_preview_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = fixture_ride(&fixture.directory, &roster)?;
    let csv = fixture.directory.path().join("entries.csv");
    std::fs::write(
        &csv,
        "FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\n\
Katherine,Johnson,solo,,111,,\nDorothy,Vaughan,team,Beta,112,,\nDorothy,Vaughan,team,Beta,113,,\n",
    )?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.open_csv_preview(&csv)?;
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("csv_preview");
    Ok(())
}

#[test]
fn test_ui_rider_issues_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = issues_roster()?;
    let (db, id) = fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.command("mi_check_rider_issues")?;
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("rider_issues");
    Ok(())
}
