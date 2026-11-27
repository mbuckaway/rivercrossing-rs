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

use craft::egui::{FontFamily, FontInsert, FontPriority, InsertFontFamily, vec2};
use craft::host::target::ShellInfo;
use craft::ui::InstallSpec;
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::shell::app::ShellApp;
use craft::ui::shell::embed;
use craft::ui::testing::windows::NoWindows;
use egui_kittest::Harness;
use tempfile::TempDir;

use rivercrossing_ride::{
    EntryMode, PlateModel, RideConfig, RideConfigParams, RideEngine, RideStatus, SystemClock,
};
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::{Entry, Rider};
use rivercrossing_store::{AuditRow, RideRow, Store};
use rivercrossing_ui_egui::ui::app::{AppPaths, RiverCrossingApp};

/// The headless app and the temporary directory its files live in.
struct GoldenFixture {
    /// The app under test.
    app: Rc<RefCell<RiverCrossingApp>>,
    /// The directory that holds the settings file and the database.
    directory: TempDir,
}

/// A headless app over a fresh temporary directory with the light appearance.
fn golden_app() -> Result<GoldenFixture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let paths = AppPaths::at(
        directory.path().join("settings.json"),
        directory.path().join("rides.db"),
        directory.path().to_path_buf(),
    );
    let mut settings = rivercrossing_store::default_settings();
    "light".clone_into(&mut settings.appearance);
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
            if let Some(emoji) = craft::ui::fonts::noto_emoji() {
                // The suit glyphs on a crossing's card, as a last-resort fallback.
                let families = [FontFamily::Proportional, FontFamily::Monospace]
                    .map(|family| InsertFontFamily {
                        family,
                        priority: FontPriority::Lowest,
                    })
                    .to_vec();
                ctx.add_font(FontInsert::new("NotoEmoji-Regular", emoji, families));
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
    // The golden frame is a still: turn egui's animation clock off so a dialog or notice that eases
    // in settles instead of requesting a pass forever.
    for theme in [craft::egui::Theme::Light, craft::egui::Theme::Dark] {
        harness
            .ctx
            .style_mut_of(theme, |style| style.animation_time = 0.0);
    }
    // Several dialogs legitimately repaint (progress, focus), so settle by stepping a fixed number
    // of frames rather than requiring quiescence. Animation time is off above, so the frame is stable.
    harness.run_steps(90);
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

/// Create a fixture ride over *roster*, start it at the epoch, record one crossing and stop it.
///
/// Every instant is pinned, and the stopped clock reads zero, so the frame renders the same on
/// every run.
fn started_fixture_ride(
    directory: &TempDir,
    roster: &Roster,
) -> Result<(std::path::PathBuf, i64), Box<dyn Error>> {
    let path = directory.path().join("started.db");
    let mut store = Store::open(&path)?;
    let id = store.create_ride(&fixture_config()?, Some(7))?;
    store.save_roster(id, roster)?;
    let mut engine: RideEngine<Roster, SystemClock> = store.load_engine(id)?;
    let date = time::Date::from_calendar_date(1970, time::Month::January, 1)?;
    let start = time::OffsetDateTime::new_utc(date, time::Time::MIDNIGHT);
    engine.start(Some(start))?;
    let _ = engine.record_crossing("101", Some(start + time::Duration::hours(1)));
    engine.stop()?;
    for event in engine.events() {
        store.append(id, event)?;
    }
    Ok((path, id))
}

/// One ride-library row to show in a golden.
struct RowSpec {
    /// The stored ride id.
    id: i64,
    /// The ride's name.
    name: &'static str,
    /// The lifecycle status.
    status: RideStatus,
    /// The live entry count.
    entries: u32,
}

/// Return the ride-library row that *spec* names.
fn ride_row(spec: &RowSpec) -> Result<RideRow, Box<dyn Error>> {
    Ok(RideRow {
        id: spec.id,
        name: spec.name.to_owned(),
        event_date: time::Date::from_calendar_date(2026, time::Month::September, 20)?,
        status: spec.status,
        entries: spec.entries,
    })
}

#[test]
fn test_ui_ride_library_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let rows = vec![
        ride_row(&RowSpec {
            id: 1,
            name: "GORBA EPIC 2026",
            status: RideStatus::Finished,
            entries: 3,
        })?,
        ride_row(&RowSpec {
            id: 2,
            name: "Club poker night",
            status: RideStatus::Draft,
            entries: 0,
        })?,
    ];

    // Act
    fixture.app.borrow_mut().show_library(rows);
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("ride_library");
    Ok(())
}

#[test]
fn test_ui_delete_ride_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let row = ride_row(&RowSpec {
        id: 1,
        name: "GORBA EPIC 2026",
        status: RideStatus::Finished,
        entries: 3,
    })?;

    // Act
    fixture.app.borrow_mut().show_delete_ride(row);
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("delete_ride");
    Ok(())
}

#[test]
fn test_ui_settings_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;

    // Act
    fixture.app.borrow_mut().command("wxID_PREFERENCES")?;
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("settings");
    Ok(())
}

#[test]
fn test_ui_audit_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = fixture_ride(&fixture.directory, &roster)?;
    let date = time::Date::from_calendar_date(2026, time::Month::September, 20)?;
    let at = time::OffsetDateTime::new_utc(date, time::Time::from_hms(14, 5, 9)?);
    let rows = vec![
        AuditRow {
            at,
            action: "crossing".to_owned(),
            entry: "101".to_owned(),
            reason: String::new(),
        },
        AuditRow {
            at: at - time::Duration::minutes(4),
            action: "void_card".to_owned(),
            entry: "Alpha".to_owned(),
            reason: "misdeal".to_owned(),
        },
    ];

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_audit(rows);
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("audit");
    Ok(())
}

#[test]
fn test_ui_selftest_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;

    // Act
    fixture.app.borrow_mut().command("mi_selftest")?;
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("selftest");
    Ok(())
}

#[test]
fn test_ui_about_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;

    // Act
    fixture.app.borrow_mut().command("wxID_ABOUT")?;
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("about");
    Ok(())
}

#[test]
fn test_ui_shortcuts_match_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;

    // Act
    fixture.app.borrow_mut().command("mi_shortcuts")?;
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("shortcuts");
    Ok(())
}

#[test]
fn test_ui_crossing_detail_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = started_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        assert!(app.open_first_crossing_detail());
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("crossing_detail");
    Ok(())
}

/// Create a fixture ride over *roster*, start it at the epoch, record three crossings and stop it.
///
/// The stopped clock reads zero, so the console behind the dialog renders the same on every run.
fn stopped_fixture_ride(
    directory: &TempDir,
    roster: &Roster,
) -> Result<(std::path::PathBuf, i64), Box<dyn Error>> {
    let path = directory.path().join("stopped.db");
    let mut store = Store::open(&path)?;
    let id = store.create_ride(&fixture_config()?, Some(7))?;
    store.save_roster(id, roster)?;
    let mut engine: RideEngine<Roster, SystemClock> = store.load_engine(id)?;
    let date = time::Date::from_calendar_date(1970, time::Month::January, 1)?;
    let start = time::OffsetDateTime::new_utc(date, time::Time::MIDNIGHT);
    engine.start(Some(start))?;
    let _ = engine.record_crossing("101", Some(start + time::Duration::minutes(40)));
    let _ = engine.record_crossing("A1", Some(start + time::Duration::minutes(50)));
    let _ = engine.record_crossing("101", Some(start + time::Duration::minutes(80)));
    engine.stop()?;
    for event in engine.events() {
        store.append(id, event)?;
    }
    Ok((path, id))
}

#[test]
fn test_ui_results_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.command("mi_standings")?;
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("results");
    Ok(())
}

#[test]
fn test_ui_publish_form_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_publish();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("publish_form");
    Ok(())
}

#[test]
fn test_ui_simulation_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.command("mi_simulation")?;
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("simulation");
    Ok(())
}

/// Create a fixture ride over *roster* with no entries, and return its database and id.
fn empty_roster_fixture_ride(
    directory: &TempDir,
) -> Result<(std::path::PathBuf, i64), Box<dyn Error>> {
    let path = directory.path().join("empty.db");
    let mut store = Store::open(&path)?;
    let id = store.create_ride(&fixture_config()?, Some(7))?;
    let roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(7))?;
    store.save_roster(id, &roster)?;
    Ok((path, id))
}

#[test]
fn test_ui_start_blocked_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let (db, id) = empty_roster_fixture_ride(&fixture.directory)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_start_blocked();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("start_blocked");
    Ok(())
}

#[test]
fn test_ui_resume_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_resume();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("resume");
    Ok(())
}

#[test]
fn test_ui_exit_running_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.command("wxID_EXIT")?;
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("exit_running");
    Ok(())
}

#[test]
fn test_ui_set_start_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_set_start();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("set_start");
    Ok(())
}

#[test]
fn test_ui_add_rider_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_add_rider();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("add_rider");
    Ok(())
}

#[test]
fn test_ui_add_team_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_add_team();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("add_team");
    Ok(())
}

#[test]
fn test_ui_edit_crossing_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_edit_crossing();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("edit_crossing");
    Ok(())
}

#[test]
fn test_ui_manual_deal_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_manual_deal();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("manual_deal");
    Ok(())
}

#[test]
fn test_ui_dnf_confirm_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_dnf();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("dnf_confirm");
    Ok(())
}

#[test]
fn test_ui_void_card_confirm_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        assert!(app.open_first_crossing_detail());
        app.show_void_card();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("void_card_confirm");
    Ok(())
}

#[test]
fn test_ui_crossing_number_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        assert!(app.open_first_crossing_detail());
        app.show_crossing_number();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("crossing_number");
    Ok(())
}

#[test]
fn test_ui_publish_running_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = stopped_fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_publish_running();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("publish_running");
    Ok(())
}

#[test]
fn test_ui_sim_running_matches_the_golden() -> Result<(), Box<dyn Error>> {
    // Arrange
    let fixture = golden_app()?;
    let roster = editor_roster()?;
    let (db, id) = fixture_ride(&fixture.directory, &roster)?;

    // Act
    {
        let mut app = fixture.app.borrow_mut();
        app.load_ride_from(&db, id)?;
        app.show_sim_running();
    }
    let mut harness = build_harness(&fixture.app)?;

    // Assert
    harness.snapshot("sim_running");
    Ok(())
}
