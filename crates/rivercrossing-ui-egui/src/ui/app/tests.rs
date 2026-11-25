// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use craft::egui::{Context, RawInput, Theme, Vec2, ViewportCommand, ViewportId};
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::runtime::access::Role;
use craft::ui::shell::app::ShellApp;
use craft::ui::shell::embed::CloseDecision;
use craft::ui::testing::Fixture;
use craft::ui::testing::access::{AccessTree, violations};
use rivercrossing_ride::{
    EntryMode, PlateModel, RideConfig, RideConfigParams, RideEngine, SystemClock,
};
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::Rider;
use rivercrossing_store::Store;
use tempfile::TempDir;

use super::{AppPaths, RiverCrossingApp};
use crate::ui::action::id::{
    MI_BACKUP_NOW, MI_NEW_RIDE, MI_OPEN_LIBRARY, MI_RIDER_EDITOR, MI_SHORTCUTS, MI_START_RIDE,
    MI_TIME_BOARD, MI_UNDO_CROSSING, MI_ZOOM_100, WX_ID_ABOUT,
};
use crate::ui::state::LoadedRide;
use crate::ui::views::menu;
use crate::ui::worker::StoreWorker;

/// The ride name every fixture uses.
const RIDE_NAME: &str = "GORBA EPIC 2026";

/// The plate that the fixture's solo entry carries.
const PLATE: &str = "101";

/// The seed every fixture ride stores.
const SEED: u64 = 7;

// ------------------------------------------------------------- fixtures

/// Return explicit per-test paths under *directory*.
fn paths_in(directory: &TempDir) -> AppPaths {
    AppPaths::at(
        directory.path().join("settings.json"),
        directory.path().join("rides.db"),
        directory.path().to_path_buf(),
    )
}

/// Return a fixture ride config whose minimum lap makes the first crossing a short lap.
fn fixture_config() -> Result<RideConfig, String> {
    RideConfig::new(RideConfigParams {
        name: RIDE_NAME.to_owned(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        min_lap_s: 1200,
        ..RideConfigParams::default()
    })
    .map_err(|error| error.to_string())
}

/// Return a roster that holds one solo entry on `PLATE`.
fn fixture_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster
        .create_solo_entry(
            Rider {
                first_name: "Ada".to_owned(),
                last_name: "Lovelace".to_owned(),
                ..Rider::default()
            },
            PLATE,
        )
        .map_err(|error| error.to_string())?;
    Ok(roster)
}

/// Build a running ride from a fresh database in *directory*.
fn loaded_running_ride(directory: &TempDir) -> Result<LoadedRide, String> {
    let mut store =
        Store::open(directory.path().join("fixture.db")).map_err(|error| error.to_string())?;
    let config = fixture_config()?;
    let ride_id = store
        .create_ride(&config, Some(SEED))
        .map_err(|error| error.to_string())?;
    let roster = fixture_roster()?;
    store
        .save_roster(ride_id, &roster)
        .map_err(|error| error.to_string())?;
    let mut engine: RideEngine<Roster, SystemClock> = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;
    engine.start(None).map_err(|error| error.to_string())?;
    Ok(LoadedRide {
        ride_id,
        config: engine.config().clone(),
        roster,
        engine,
    })
}

/// A fixture over a headless app, with the directory kept alive.
fn app_fixture(app: &Rc<RefCell<RiverCrossingApp>>) -> Fixture {
    let ui_app = Rc::clone(app);
    let mut fixture = Fixture::new(move |ui| ui_app.borrow_mut().ui(ui));
    fixture.room(Vec2::new(1100.0, 780.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture
}

/// Build a headless app over a fresh temporary directory.
fn headless_app(quit_after: Option<Duration>) -> (Rc<RefCell<RiverCrossingApp>>, TempDir) {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let app = RiverCrossingApp::headless(quit_after, None, paths_in(&directory));
    (Rc::new(RefCell::new(app)), directory)
}

/// Run one logic pass, then apply the worker's replies until the ride is loaded.
///
/// The worker answers the launch's `PreviousSession` first, then the create, then the load.
fn drain_until_ride(app: &Rc<RefCell<RiverCrossingApp>>, ctx: &Context) {
    app.borrow_mut().logic(ctx);
    for _ in 0..3 {
        let reply = app.borrow().worker.as_ref().and_then(StoreWorker::recv);
        let Some(reply) = reply else {
            return;
        };
        app.borrow_mut().apply_reply(reply);
        if app.borrow().state.ride.is_some() {
            return;
        }
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

// ------------------------------------------------------------- the shell

#[test]
fn app_ui_renders_one_frame_without_panicking() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let ctx = Context::default();

    // Act
    let output = ctx.run_ui(RawInput::default(), |ui| app.borrow_mut().ui(ui));
    let rendered_root = output.viewport_output.contains_key(&ViewportId::ROOT);

    // Assert
    assert!(rendered_root);
    output.drop_without_applying_deltas();
}

#[test]
fn app_close_request_is_close() {
    // Arrange
    let (app, _directory) = headless_app(None);

    // Act
    let decision = app.borrow_mut().close_requested();

    // Assert
    assert_eq!(decision, CloseDecision::Close);
}

#[test]
fn app_logic_emits_viewport_close_when_the_quit_deadline_passes() {
    // Arrange
    let (app, _directory) = headless_app(Some(Duration::ZERO));
    let ctx = Context::default();

    // Act
    let output = ctx.run_logic(&RawInput::default(), |ctx| app.borrow_mut().logic(ctx));

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
    let (app, _directory) = headless_app(None);
    let ctx = Context::default();

    // Act
    let output = ctx.run_logic(&RawInput::default(), |ctx| app.borrow_mut().logic(ctx));

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
    let (app, _directory) = headless_app(Some(Duration::from_secs(3600)));
    let ctx = Context::default();

    // Act
    let output = ctx.run_logic(&RawInput::default(), |ctx| app.borrow_mut().logic(ctx));

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
    let (app, _directory) = headless_app(Some(Duration::MAX));
    let ctx = Context::default();

    // Act
    let output = ctx.run_logic(&RawInput::default(), |ctx| app.borrow_mut().logic(ctx));

    // Assert
    let closes = output
        .viewport_commands
        .get(&ViewportId::ROOT)
        .is_some_and(|commands| commands.contains(&ViewportCommand::Close));
    assert!(!closes, "an overflowing deadline must not close the window");
}

#[test]
fn app_tick_requests_a_frame_at_the_deadline() {
    // Arrange
    let (app, _directory) = headless_app(Some(Duration::from_secs(60)));
    let now = std::time::Instant::now();

    // Act
    let due = app.borrow_mut().tick(now);

    // Assert
    assert!(due.is_some_and(|deadline| deadline > now));
}

#[test]
fn app_tick_given_no_deadline_requests_nothing() {
    // Arrange
    let (app, _directory) = headless_app(None);

    // Act
    let due = app.borrow_mut().tick(std::time::Instant::now());

    // Assert
    assert_eq!(due, None);
}

#[test]
fn test_app_logic_applies_the_saved_zoom() {
    // Arrange
    let directory = tempfile::tempdir().expect("a temporary directory");
    let paths = paths_in(&directory);
    let mut settings = rivercrossing_store::default_settings();
    settings.zoom_percent = 130;
    rivercrossing_store::save_settings(&settings, &paths.settings).expect("the settings write");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(None, None, paths)));
    let ctx = Context::default();

    // Act: the new zoom takes effect at the start of the next pass.
    let _first = ctx.run_logic(&RawInput::default(), |ctx| app.borrow_mut().logic(ctx));
    let pass = ctx.run_ui(RawInput::default(), |_ui| {});

    // Assert
    assert!((ctx.zoom_factor() - 1.3).abs() < 1e-6, "the zoom is 130%");
    assert_eq!(app.borrow().applied_zoom, Some(130));
    pass.drop_without_applying_deltas();
}

#[test]
fn test_app_logic_applies_the_dark_appearance() {
    // Arrange
    let directory = tempfile::tempdir().expect("a temporary directory");
    let paths = paths_in(&directory);
    let mut settings = rivercrossing_store::default_settings();
    settings.appearance = "dark".to_owned();
    rivercrossing_store::save_settings(&settings, &paths.settings).expect("the settings write");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(None, None, paths)));
    let ctx = Context::default();

    // Act
    let _output = ctx.run_logic(&RawInput::default(), |ctx| app.borrow_mut().logic(ctx));

    // Assert
    assert_eq!(ctx.theme(), Theme::Dark);
}

// ------------------------------------------------------------- the menu

#[test]
fn test_the_menu_registry_has_53_rows() {
    // Arrange
    let (app, _directory) = headless_app(None);

    // Act
    let rows = menu::rows(&app.borrow().state);

    // Assert
    assert_eq!(rows.len(), 53);
    assert_eq!(
        rows.iter().filter(|row| row.enabled).count(),
        20,
        "the empty console enables twenty rows"
    );
}

#[test]
fn test_the_menu_disables_the_rows_the_gate_or_the_release_refuses() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let app_guard = app.borrow();
    let state = &app_guard.state;

    // Act
    let rows = menu::rows(state);

    // Assert
    let enabled = |id| {
        rows.iter()
            .find(|row| row.id == id)
            .is_some_and(|row| row.enabled)
    };
    assert!(enabled(MI_NEW_RIDE), "New Ride is enabled with no ride");
    assert!(enabled(MI_BACKUP_NOW), "Back Up Database is always enabled");
    assert!(enabled(MI_SHORTCUTS), "the shortcuts dialog is built");
    assert!(enabled(MI_ZOOM_100), "the zoom rows are always enabled");
    assert!(enabled(WX_ID_ABOUT), "the about dialog is built");
    assert!(!enabled(MI_OPEN_LIBRARY), "the ride library is not built");
    assert!(!enabled(MI_START_RIDE), "Start Ride needs a ride");
    assert!(!enabled(MI_UNDO_CROSSING), "Undo needs a crossing");
    assert!(!enabled(MI_RIDER_EDITOR), "the rider editor is not built");
    assert!(
        !enabled(MI_TIME_BOARD),
        "the time board needs show-times on"
    );
}

#[test]
fn test_the_menu_shows_the_saved_checks_and_zoom() {
    // Arrange
    let directory = tempfile::tempdir().expect("a temporary directory");
    let paths = paths_in(&directory);
    let mut settings = rivercrossing_store::default_settings();
    settings.zoom_percent = 110;
    settings.publish_laps_board = false;
    rivercrossing_store::save_settings(&settings, &paths.settings).expect("the settings write");
    let app = RiverCrossingApp::headless(None, None, paths);

    // Act
    let rows = menu::rows(&app.state);

    // Assert
    let row = |id| rows.iter().find(|row| row.id == id).cloned();
    assert_eq!(
        row(crate::ui::action::id::MI_LAPS_BOARD).and_then(|row| row.checked),
        Some(false)
    );
    assert_eq!(
        row(crate::ui::action::id::MI_ZOOM_110).and_then(|row| row.radio),
        Some(true)
    );
    assert_eq!(
        row(crate::ui::action::id::MI_ZOOM_100).and_then(|row| row.radio),
        Some(false)
    );
}

// ---------------------------------------------------------- the console

#[test]
fn test_the_console_empty_state_shows_the_zero_clock_and_disables_the_entry() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);

    // Act
    fixture.idle(2);

    // Assert
    let tree = fixture.tree();
    let zeros = tree
        .keys()
        .into_iter()
        .filter(|key| {
            tree.name_of(*key) == Some("0:00:00") || tree.value_of(*key) == Some("0:00:00")
        })
        .count();
    assert_eq!(zeros, 2, "both clocks read zero");
    assert!(tree_mentions(tree, "NO RIDE"), "the status label spells it");
    assert!(tree_mentions(tree, "No crossings yet"));
    let plate = fixture.by_label("Plate").expect("the plate field");
    assert!(tree.is_disabled_key(plate.node), "the entry is disabled");
    let record = fixture.by_label("Record").expect("the record button");
    assert!(
        tree.is_disabled_key(record.node),
        "record follows the entry"
    );
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
}

#[test]
fn test_the_console_menu_bar_lists_the_seven_titles() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);

    // Act
    fixture.idle(2);

    // Assert
    for title in ["File", "Ride", "Riders", "Cards", "Results", "View", "Help"] {
        assert!(
            fixture.by_label(title).is_some(),
            "the {title} title is in the bar"
        );
    }
}

#[test]
fn test_the_record_button_records_a_typed_plate() {
    // Arrange
    let directory = tempfile::tempdir().expect("a temporary directory");
    let ride = loaded_running_ride(&directory).expect("the fixture ride");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let field = fixture.by_label("Plate").expect("the plate field");
    let unfocused = fixture.tree().is_disabled_key(field.node);
    assert!(!unfocused, "a running ride accepts an entry");

    // Act
    fixture.click(&field);
    fixture.type_text(PLATE);
    let record = fixture.by_label("Record").expect("the record button");
    fixture.click(&record);
    fixture.idle(1);

    // Assert
    let app_guard = app.borrow();
    let state = &app_guard.state;
    assert_eq!(
        state.notice.as_deref(),
        Some("Recorded plate 101 — short lap flagged for review")
    );
    assert_eq!(
        state
            .ride
            .as_ref()
            .map(|ride| ride.engine.crossings().len()),
        Some(1)
    );
    assert_eq!(state.plate, "");
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
}

#[test]
fn test_the_start_button_starts_a_draft_ride() {
    // Arrange
    let directory = tempfile::tempdir().expect("a temporary directory");
    let mut store = Store::open(directory.path().join("fixture.db")).expect("the fixture store");
    let config = fixture_config().expect("the fixture config");
    let ride_id = store.create_ride(&config, Some(SEED)).expect("the ride");
    let roster = fixture_roster().expect("the fixture roster");
    store
        .save_roster(ride_id, &roster)
        .expect("the roster write");
    let engine = store.load_engine(ride_id).expect("the ride engine");
    let ride = LoadedRide {
        ride_id,
        config: engine.config().clone(),
        roster,
        engine,
    };
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let start = fixture.by_label("Start Ride").expect("the start button");

    // Act: the click queues the action, and a logic pass runs it.
    fixture.click(&start);
    let ctx = fixture.ctx().clone();
    app.borrow_mut().logic(&ctx);
    fixture.idle(1);

    // Assert
    let app_guard = app.borrow();
    let state = &app_guard.state;
    assert_eq!(
        state.notice.as_deref(),
        Some(crate::ui::presenters::console::RIDE_STARTED)
    );
    assert_eq!(state.phase(), crate::ui::action::enable::RidePhase::Running);
}

#[test]
fn test_the_new_ride_command_opens_the_setup_dialog() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    let opened = app.borrow_mut().command("mi_new_ride");
    assert_eq!(opened, Ok(serde_json::json!({"ok": true})));

    // Act
    fixture.idle(2);

    // Assert
    assert!(tree_mentions(fixture.tree(), "Ride Setup"));
    assert!(tree_mentions(fixture.tree(), "Ride name"));
    assert_eq!(
        app.borrow()
            .state
            .dialogs
            .field("date_picker")
            .map(str::len),
        Some(10)
    );
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
}

#[test]
fn test_the_setup_ok_persists_the_ride_and_loads_it() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let ctx = Context::default();
    app.borrow_mut().command("mi_new_ride").expect("the dialog");
    {
        let mut app = app.borrow_mut();
        for (field, value) in [
            ("name_input", RIDE_NAME),
            ("venue_input", "Sea to Sky Gondola"),
            ("organizer_input", "GORBA"),
            ("scorer_input", "K. Singh"),
        ] {
            app.state
                .dialogs
                .set_field(field, value)
                .expect("the setup field");
        }
        app.state.submit_setup().expect("a complete setup");
        assert!(app.state.pending_create.is_some());
    }

    // Act: the logic pass persists the ride, and the worker's replies load it back.
    drain_until_ride(&app, &ctx);

    // Assert
    let app_guard = app.borrow();
    let state = &app_guard.state;
    assert_eq!(state.ride.as_ref().map(|ride| ride.ride_id), Some(1));
    assert_eq!(
        state.ride.as_ref().map(|ride| ride.config.name.clone()),
        Some(RIDE_NAME.to_owned())
    );
    assert_eq!(state.audit_row_count, 0);
}

#[test]
fn test_apply_reply_posts_a_store_error_as_the_notice() {
    // Arrange
    let (app, _directory) = headless_app(None);

    // Act
    app.borrow_mut()
        .apply_reply(Err("the database refused".to_owned()));

    // Assert
    assert_eq!(
        app.borrow().state.notice.as_deref(),
        Some("the database refused")
    );
}

#[test]
fn test_a_setup_refusal_keeps_the_dialog_open() {
    // Arrange
    let (app, _directory) = headless_app(None);
    app.borrow_mut().command("mi_new_ride").expect("the dialog");
    let mut fixture = app_fixture(&app);
    fixture.idle(1);
    {
        let mut app = app.borrow_mut();
        app.state
            .dialogs
            .set_field("date_picker", "not a date")
            .expect("the date field");
    }

    // Act
    let result = app.borrow_mut().state.submit_setup();

    // Assert
    assert!(result.is_err());
    assert_eq!(
        app.borrow().state.dialogs.top().map(|dialog| dialog.id),
        Some("ride_setup_dlg")
    );
    assert!(app.borrow().state.pending_create.is_none());
}

#[test]
fn test_the_fixture_reaches_the_console_by_label() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    fixture.idle(2);

    // Act
    let target = fixture
        .by_label("Search crossings")
        .expect("the search field carries its name");

    // Assert
    assert_eq!(fixture.tree().role_of(target.node), Some(Role::TEXT_INPUT));
    assert!(target.rect.width() > 0.0 && target.rect.height() > 0.0);
}

// ------------------------------------------------------- native confirms

/// Build a running app over *directory* whose ride holds one crossing.
fn app_with_a_crossing(
    directory: &TempDir,
) -> Result<Rc<RefCell<RiverCrossingApp>>, Box<dyn std::error::Error>> {
    let mut ride = loaded_running_ride(directory)?;
    let _ = ride.engine.record_crossing(PLATE, None);
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    Ok(app)
}

#[test]
fn test_a_native_confirm_does_not_run_the_command_until_confirmed()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = app_with_a_crossing(&directory)?;

    // Act
    let opened = app.borrow_mut().command("mi_undo_crossing")?;

    // Assert: the action queues a confirmation, and nothing has run yet.
    assert_eq!(opened, serde_json::json!({"ok": true}));
    {
        let guard = app.borrow();
        assert!(guard.state.pending_native.is_some());
        assert_eq!(guard.state.pending_command, None);
        assert_eq!(
            guard
                .state
                .ride
                .as_ref()
                .map(|ride| ride.engine.crossings().len()),
            Some(1)
        );
    }
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    assert!(tree_mentions(fixture.tree(), "Undo the last crossing?"));
    assert!(fixture.by_label("Cancel").is_some());
    assert!(fixture.by_label("Confirm Undo").is_some());
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn test_cancelling_a_native_confirm_runs_nothing() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = app_with_a_crossing(&directory)?;
    app.borrow_mut().command("mi_undo_crossing")?;
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let cancel = fixture.by_label("Cancel").expect("the cancel button");

    // Act
    fixture.click(&cancel);
    fixture.idle(1);

    // Assert
    let guard = app.borrow();
    assert_eq!(guard.state.pending_native, None);
    assert_eq!(guard.state.pending_command, None);
    assert_eq!(
        guard
            .state
            .ride
            .as_ref()
            .map(|ride| ride.engine.crossings().len()),
        Some(1),
        "the crossing survives a cancelled undo"
    );
    Ok(())
}

#[test]
fn test_confirming_a_native_confirm_forwards_the_command() -> Result<(), Box<dyn std::error::Error>>
{
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = app_with_a_crossing(&directory)?;
    app.borrow_mut().command("mi_undo_crossing")?;
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let confirm = fixture
        .by_label("Confirm Undo")
        .expect("the confirm button");

    // Act: the click queues the command, and a logic pass runs it.
    fixture.click(&confirm);
    fixture.idle(1);
    let ctx = fixture.ctx().clone();
    app.borrow_mut().logic(&ctx);
    fixture.idle(1);

    // Assert
    let guard = app.borrow();
    assert_eq!(guard.state.pending_native, None);
    assert_eq!(guard.state.pending_command, None);
    assert_eq!(
        guard
            .state
            .ride
            .as_ref()
            .map(|ride| ride.engine.crossings().len()),
        Some(0),
        "the confirmed undo removed the crossing"
    );
    Ok(())
}

#[test]
fn test_the_stop_button_confirms_through_the_modal() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let ride = loaded_running_ride(&directory)?;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let stop = fixture.by_label("Stop Ride").expect("the stop button");

    // Act: the console button raises the same confirmation.
    fixture.click(&stop);
    fixture.idle(2);
    let confirm = fixture
        .by_label("Confirm Stop")
        .expect("the stop confirmation");
    fixture.click(&confirm);
    fixture.idle(1);
    let ctx = fixture.ctx().clone();
    app.borrow_mut().logic(&ctx);
    fixture.idle(1);

    // Assert
    let guard = app.borrow();
    assert_eq!(guard.state.pending_native, None);
    assert_eq!(
        guard.state.phase(),
        crate::ui::action::enable::RidePhase::RunningStopped
    );
    assert_eq!(
        guard.state.notice.as_deref(),
        Some("Ride stopped — continue to resume")
    );
    Ok(())
}
