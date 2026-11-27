// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use craft::dialogs::{Answer, FileDialog, Request};
use craft::egui::{
    Context, Event, Key as EguiKey, Modifiers, RawInput, Theme, ThemePreference, Vec2,
    ViewportCommand, ViewportId,
};
use craft::host::target::Os;
use craft::theme::{ContrastChoice, ModeChoice, Size, TextSize};
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::runtime::access::Role;
use craft::ui::shell::app::ShellApp;
use craft::ui::shell::embed::CloseDecision;
use craft::ui::testing::access::{AccessTree, violations};
use craft::ui::testing::focus::AccessActionKind;
use craft::ui::testing::{Fixture, Target};
use rivercrossing_ride::{
    EntryMode, PlateModel, RideConfig, RideConfigParams, RideEngine, RideStatus, SystemClock,
};
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::{Entry, Rider};
use rivercrossing_store::Store;
use rstest::rstest;
use tempfile::TempDir;

use super::{AppPaths, RIDE_UPDATED, RiverCrossingApp};
use crate::ui::action::effect::{DispatchError, Effect};
use crate::ui::action::enable::{RidePhase, enabled};
use crate::ui::action::id::{
    MI_AUDIT_TRAIL, MI_BACKUP_NOW, MI_DENSITY_COMFORTABLE, MI_DENSITY_COMPACT, MI_NEW_RIDE,
    MI_OPEN_LIBRARY, MI_RIDER_EDITOR, MI_SHORTCUTS, MI_START_RIDE, MI_TEXT_SIZE_LARGE,
    MI_TIME_BOARD, MI_UNDO_CROSSING, WX_ID_ABOUT,
};
use crate::ui::action::spec::{ACTIONS, CommandId, NativeDialog, Target as SpecTarget};
use crate::ui::help::{MockBrowserLauncher, USER_GUIDE, USER_GUIDE_FILE};
use crate::ui::presenters::console::{
    PLATE_LABEL, RECORD_LABEL, RIDE_STARTED, START_RIDE_LABEL, STOP_RIDE_LABEL,
};
use crate::ui::presenters::settings_form;
use crate::ui::state::{
    AppState, ConsoleTab, ExportPresence, FILE_PICKER_UNAVAILABLE, LoadedRide, NativeTask,
    PendingNative,
};
use crate::ui::views::menu;
use crate::ui::worker::{StoreCommand, StoreReply, StoreWorker};

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

/// Build a draft ride with *config* from a fresh database in *directory*.
fn draft_ride_with(directory: &TempDir, config: &RideConfig) -> Result<LoadedRide, String> {
    let mut store =
        Store::open(directory.path().join("fixture.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(config, Some(SEED))
        .map_err(|error| error.to_string())?;
    let roster = fixture_roster()?;
    store
        .save_roster(ride_id, &roster)
        .map_err(|error| error.to_string())?;
    let engine: RideEngine<Roster, SystemClock> = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;
    Ok(LoadedRide {
        ride_id,
        config: engine.config().clone(),
        roster,
        engine,
    })
}

/// Build a draft ride from a fresh database in *directory*.
fn loaded_draft_ride(directory: &TempDir) -> Result<LoadedRide, String> {
    draft_ride_with(directory, &fixture_config()?)
}

/// Build a running ride from a fresh database in *directory*.
fn loaded_running_ride(directory: &TempDir) -> Result<LoadedRide, String> {
    let mut ride = loaded_draft_ride(directory)?;
    ride.engine.start(None).map_err(|error| error.to_string())?;
    Ok(ride)
}

/// Build a running ride that credits every short lap's card, from a fresh database in *directory*.
fn loaded_always_deal_ride(directory: &TempDir) -> Result<LoadedRide, String> {
    let mut config = fixture_config()?;
    config.hold_short_laps = false;
    let mut ride = draft_ride_with(directory, &config)?;
    ride.engine.start(None).map_err(|error| error.to_string())?;
    Ok(ride)
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
/// The worker answers the launch's `PreviousSession`, then its `Rides`, then the startup `Backup`,
/// then the create's `RideCreated`, then the load's `Ride`: five replies. `logic` drains whichever
/// are already queued, so the loop reads the rest; a shorter budget loses the load reply whenever
/// the worker has not answered by the time `logic` runs.
fn drain_until_ride(app: &Rc<RefCell<RiverCrossingApp>>, ctx: &Context) {
    app.borrow_mut().logic(ctx);
    for _ in 0..5 {
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
    // Arrange: the console runs on the craft shell, so the frame needs the installed runtime that the
    // fixture provides.
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);

    // Act
    fixture.idle(2);

    // Assert
    assert!(
        fixture.by_label(PLATE_LABEL).is_some(),
        "the console rendered its plate field"
    );
}

#[rstest]
#[case::this_platform(Os::current())]
fn app_close_request_holds_and_asks_first(#[case] os: Os) {
    // Arrange
    let (app, _directory) = headless_app(None);

    // Act
    let decision = app.borrow_mut().close_requested();

    // Assert: the window holds. macOS hides it without a prompt; Windows and Linux raise the quit
    // confirm.
    assert_eq!(decision, CloseDecision::Hold);
    assert_eq!(app.borrow().minimize_next_frame, os == Os::Mac);
    assert_eq!(app.borrow().state.pending_native.is_some(), os != Os::Mac);
}

#[test]
fn app_close_request_closes_after_the_quit_is_confirmed() {
    // Arrange
    let (app, _directory) = headless_app(None);
    app.borrow_mut().quit_confirmed = true;

    // Act
    let decision = app.borrow_mut().close_requested();

    // Assert
    assert_eq!(decision, CloseDecision::Close);
}

#[test]
fn app_close_request_closes_at_the_quit_deadline() {
    // Arrange
    let (app, _directory) = headless_app(Some(Duration::from_secs(60)));

    // Act
    let decision = app.borrow_mut().close_requested();

    // Assert
    assert_eq!(decision, CloseDecision::Close);
}

#[rstest]
#[case::this_platform(Os::current())]
fn app_logic_minimizes_the_window_after_a_macos_close(#[case] os: Os) {
    // Arrange
    let (app, _directory) = headless_app(None);
    let ctx = Context::default();
    let held = app.borrow_mut().close_requested();
    assert_eq!(held, CloseDecision::Hold);

    // Act: the next pass after the hold, never the same frame as `hold_close`.
    let output = ctx.run_logic(&RawInput::default(), |ctx| app.borrow_mut().logic(ctx));

    // Assert
    let minimized = output
        .viewport_commands
        .get(&ViewportId::ROOT)
        .is_some_and(|commands| commands.contains(&ViewportCommand::Minimized(true)));
    assert_eq!(minimized, os == Os::Mac);
}

#[test]
fn app_logic_minimizes_the_window_once() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let ctx = Context::default();
    let _held = app.borrow_mut().close_requested();
    let _first = ctx.run_logic(&RawInput::default(), |ctx| app.borrow_mut().logic(ctx));

    // Act
    let output = ctx.run_logic(&RawInput::default(), |ctx| app.borrow_mut().logic(ctx));

    // Assert
    assert!(
        !output
            .viewport_commands
            .get(&ViewportId::ROOT)
            .is_some_and(|commands| commands.contains(&ViewportCommand::Minimized(true))),
        "the minimize request is sent once"
    );
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
fn app_tick_requests_the_next_backup_hour() {
    // Arrange
    let (app, _directory) = headless_app(None);

    // Act
    let due = app.borrow_mut().tick(std::time::Instant::now());

    // Assert: the app asks for a frame at the next whole hour for its backup.
    assert!(due.is_some_and(|deadline| deadline > std::time::Instant::now()));
}

#[test]
fn test_app_frame_input_maps_the_saved_appearance() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let paths = paths_in(&directory);
    let mut settings = rivercrossing_store::default_settings();
    settings.appearance = "dark".to_owned();
    rivercrossing_store::save_settings(&settings, &paths.settings)?;
    let mut app = RiverCrossingApp::headless(None, None, paths);

    // Act
    let input = app.frame_input();

    // Assert
    assert_eq!(input.root.mode, ModeChoice::Dark);
    Ok(())
}

#[test]
fn test_app_frame_input_maps_every_appearance_axis() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let paths = paths_in(&directory);
    let settings = rivercrossing_store::settings::AppSettings {
        appearance: "light".to_owned(),
        contrast: "high".to_owned(),
        density: "comfortable".to_owned(),
        text_size: "large".to_owned(),
        theme: "craft.studio".to_owned(),
        ..rivercrossing_store::default_settings()
    };
    rivercrossing_store::save_settings(&settings, &paths.settings)?;
    let mut app = RiverCrossingApp::headless(None, None, paths);

    // Act
    let input = app.frame_input();

    // Assert
    assert_eq!(
        (
            input.root.mode,
            input.root.contrast,
            input.root.density,
            input.root.text_size,
        ),
        (
            ModeChoice::Light,
            ContrastChoice::High,
            Size::Comfortable,
            TextSize::Large,
        )
    );
    assert_eq!(input.root.theme_id().as_str(), "craft.studio");
    Ok(())
}

#[test]
fn test_app_logic_leaves_the_egui_theme_to_the_root_env() -> Result<(), Box<dyn std::error::Error>>
{
    // Arrange
    let directory = tempfile::tempdir()?;
    let paths = paths_in(&directory);
    let mut settings = rivercrossing_store::default_settings();
    settings.appearance = "dark".to_owned();
    rivercrossing_store::save_settings(&settings, &paths.settings)?;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(None, None, paths)));
    let ctx = Context::default();
    ctx.set_theme(ThemePreference::Light);

    // Act
    let _output = ctx.run_logic(&RawInput::default(), |ctx| app.borrow_mut().logic(ctx));

    // Assert: the appearance reaches the runtime through `frame_input`, not `ctx.set_theme`.
    assert_eq!(ctx.theme(), Theme::Light);
    Ok(())
}

// ------------------------------------------------------------- the menu

#[test]
fn test_the_menu_registry_has_52_rows() {
    // Arrange
    let (app, _directory) = headless_app(None);

    // Act
    let rows = menu::rows(&app.borrow().state);

    // Assert
    assert_eq!(rows.len(), 52);
    assert_eq!(
        rows.iter().filter(|row| row.enabled).count(),
        24,
        "the empty console enables twenty-four rows"
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
    assert!(
        enabled(MI_DENSITY_COMPACT),
        "the density and text-size rows are always enabled"
    );
    assert!(enabled(WX_ID_ABOUT), "the about dialog is built");
    assert!(enabled(MI_OPEN_LIBRARY), "the ride library is built");
    assert!(!enabled(MI_START_RIDE), "Start Ride needs a ride");
    assert!(!enabled(MI_UNDO_CROSSING), "Undo needs a crossing");
    assert!(!enabled(MI_RIDER_EDITOR), "the rider editor needs a ride");
    assert!(
        !enabled(MI_TIME_BOARD),
        "the time board needs show-times on"
    );
}

#[test]
fn test_the_menu_shows_the_saved_checks_and_size_axes() {
    // Arrange
    let directory = tempfile::tempdir().expect("a temporary directory");
    let paths = paths_in(&directory);
    let mut settings = rivercrossing_store::default_settings();
    settings.density = "comfortable".to_owned();
    settings.text_size = "large".to_owned();
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
        row(MI_DENSITY_COMFORTABLE).and_then(|row| row.radio),
        Some(true)
    );
    assert_eq!(
        row(MI_DENSITY_COMPACT).and_then(|row| row.radio),
        Some(false)
    );
    assert_eq!(
        row(MI_TEXT_SIZE_LARGE).and_then(|row| row.radio),
        Some(true)
    );
}

// ---------------------------------------------------------- the native menu

#[test]
fn test_a_headless_app_keeps_the_in_window_menu_bar() {
    // Arrange
    let (app, _directory) = headless_app(None);

    // Act
    let installed = app.borrow().menubar.is_installed();

    // Assert: with no platform bar installed, `ui` renders the in-window bar.
    assert!(
        !installed,
        "the headless host installs no platform menu bar"
    );
}

#[test]
fn test_app_logic_runs_a_documented_shortcut() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let ctx = Context::default();
    let input = RawInput {
        events: vec![Event::Key {
            key: EguiKey::F5,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..RawInput::default()
    };

    // Act
    let output = ctx.run_ui(input, |ui| app.borrow_mut().logic(ui.ctx()));
    output.drop_without_applying_deltas();

    // Assert
    assert_eq!(
        app.borrow().state.dialogs.top().map(|dialog| dialog.id),
        Some("results_dlg")
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
    let plate = fixture.by_label(PLATE_LABEL).expect("the plate field");
    assert!(tree.is_disabled_key(plate.node), "the entry is disabled");
    let record = fixture.by_label(RECORD_LABEL).expect("the record button");
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
fn test_the_console_status_bar_renders_the_six_count_chips() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);

    // Act
    fixture.idle(2);

    // Assert
    let tree = fixture.tree();
    assert!(tree_mentions(tree, "Crossings 0"));
    assert!(tree_mentions(tree, "Cards dealt 0"));
    assert!(tree_mentions(tree, "On course 0"));
    assert!(tree_mentions(tree, "Shoe 0/0"));
    assert!(tree_mentions(tree, "Riders 0"));
    assert!(tree_mentions(tree, "Teams 0"));
}

#[test]
fn test_the_cards_chip_counts_a_credited_card() {
    // Arrange: a running always-deal ride with one short-lap crossing, so its card is credited.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let mut ride = loaded_always_deal_ride(&directory).expect("the fixture ride");
    let _ = ride.engine.record_crossing(PLATE, None);
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let mut fixture = app_fixture(&app);

    // Act
    fixture.idle(2);

    // Assert
    let tree = fixture.tree();
    assert!(tree_mentions(tree, "Crossings 1"));
    assert!(tree_mentions(tree, "Cards dealt 1"));
}

#[test]
fn test_the_cards_chip_skips_a_held_card() {
    // Arrange: a default holding ride with one short-lap crossing, so its card waits in the hold
    // queue.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let mut ride = loaded_running_ride(&directory).expect("the fixture ride");
    let _ = ride.engine.record_crossing(PLATE, None);
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let mut fixture = app_fixture(&app);

    // Act
    fixture.idle(2);

    // Assert
    let tree = fixture.tree();
    assert!(tree_mentions(tree, "Crossings 1"));
    assert!(tree_mentions(tree, "Cards dealt 0"));
}

#[test]
fn test_the_console_review_notebook_opens_the_needs_review_tab() {
    // Arrange: a running ride with one short-lap crossing, so the review tab has a flagged row.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let mut ride = loaded_running_ride(&directory).expect("the fixture ride");
    let _ = ride.engine.record_crossing(PLATE, None);
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let tab = fixture
        .by_label("Needs Review")
        .expect("the Needs Review tab shows");

    // Act
    fixture.click(&tab);
    fixture.idle(1);

    // Assert
    assert_eq!(app.borrow().state.console_tab, ConsoleTab::NeedsReview);
    assert!(fixture.by_label("Show held only").is_some());
    assert!(fixture.by_label("Review…").is_some());
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
}

/// Return the rail's entry for *label*.
///
/// The review notebook's first tab carries the same word as the rail's `Riders` entry, so the rail
/// is found by its column: nothing else sits left of it.
fn rail_entry(fixture: &Fixture, label: &str) -> Target {
    let edge = RAIL_COLUMN_EDGE;
    fixture
        .by_role(Role::BUTTON)
        .into_iter()
        .find(|target| {
            target.center().x < edge && fixture.tree().name_of(target.node) == Some(label)
        })
        .expect("the rail entry shows")
}

/// The x past which a control is no longer in the rail column.
const RAIL_COLUMN_EDGE: f32 = 140.0;

#[test]
fn test_dragging_a_tie_break_chip_reorders_the_list() {
    // Arrange: a fresh ride-setup form in its default order.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut()
        .state
        .open_setup_dialog(time::OffsetDateTime::now_utc().date());
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let chip = fixture.by_label("Best hand").expect("the first chip shows");

    // Act: drag it onto the next chip's place, just past its own end.
    let by = Vec2::new(chip.rect.width() / 2.0 + 2.0, 0.0);
    fixture.drag(&chip, by);
    fixture.idle(2);

    // Assert
    assert_eq!(
        app.borrow().state.setup_tiebreak,
        ["laps", "high_card", "total_time"]
    );
}

#[rstest]
#[case::library("Library", "ride_library_dlg")]
#[case::riders("Riders", "rider_editor_dlg")]
#[case::teams("Teams", "team_editor_dlg")]
#[case::issues("Issues", "rider_issues_dlg")]
#[case::standings("Standings", "results_dlg")]
#[case::audit("Audit", "audit_dlg")]
#[case::settings("Settings", "settings_dlg")]
fn test_the_console_rail_dispatches_the_action(#[case] label: &str, #[case] dialog: &str) {
    // Arrange: a running mixed ride with an audit row, so every rail action passes its gate.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let ride = loaded_running_ride(&directory).expect("the fixture ride");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    app.borrow_mut().state.audit_row_count = 1;
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let rail = rail_entry(&fixture, label);

    // Act
    fixture.click(&rail);
    fixture.idle(1);

    // Assert
    assert_eq!(
        app.borrow().state.dialogs.top().map(|dialog| dialog.id),
        Some(dialog)
    );
}

#[test]
fn test_the_muted_rail_entry_describes_the_gate() {
    // Arrange: no ride, so the Riders entry is muted.
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let riders = rail_entry(&fixture, "Riders");

    // Act
    let description = fixture.tree().description_of(riders.node);

    // Assert
    assert_eq!(description, Some("Open a ride first"));
}

#[test]
fn test_the_enabled_rail_entry_describes_no_gate() {
    // Arrange: a running mixed ride with an audit row, so every rail entry is enabled.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let ride = loaded_running_ride(&directory).expect("the fixture ride");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    app.borrow_mut().state.audit_row_count = 1;
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let riders = rail_entry(&fixture, "Riders");

    // Act
    let description = fixture.tree().description_of(riders.node);

    // Assert
    assert_eq!(description, None);
}

#[test]
fn test_the_standings_dialog_with_no_ride_offers_a_ride() {
    // Arrange
    let (app, _directory) = headless_app(None);
    app.borrow_mut().state.open_results();
    let mut fixture = app_fixture(&app);

    // Act
    fixture.idle(2);

    // Assert
    assert!(tree_mentions(
        fixture.tree(),
        "No standings yet — open a ride."
    ));
}

#[test]
fn test_the_menu_model_lists_the_seven_sections() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);

    // Act: the model is ours; the rendered labels are the platform's.
    let titles = menu::section_titles();
    fixture.idle(2);

    // Assert
    assert_eq!(
        titles,
        ["File", "Ride", "Riders", "Cards", "Results", "View", "Help"]
    );
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
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
    let field = fixture.by_label(PLATE_LABEL).expect("the plate field");
    let unfocused = fixture.tree().is_disabled_key(field.node);
    assert!(!unfocused, "a running ride accepts an entry");

    // Act
    fixture.click(&field);
    fixture.type_text(PLATE);
    let record = fixture.by_label(RECORD_LABEL).expect("the record button");
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
    let ride = loaded_draft_ride(&directory).expect("the fixture ride");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let start = fixture
        .by_label(START_RIDE_LABEL)
        .expect("the start button");

    // Act: the click queues the action, and a logic pass runs it.
    fixture.click(&start);
    let ctx = fixture.ctx().clone();
    app.borrow_mut().logic(&ctx);
    fixture.idle(1);

    // Assert
    let app_guard = app.borrow();
    let state = &app_guard.state;
    assert_eq!(state.notice.as_deref(), Some(RIDE_STARTED));
    assert_eq!(state.phase(), crate::ui::action::enable::RidePhase::Running);
}

#[test]
fn test_a_console_button_click_asks_for_the_next_frame() {
    // Arrange: a draft ride, so the console's Start Ride button is live.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let ride = loaded_draft_ride(&directory).expect("the fixture ride");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let start = fixture
        .by_label(START_RIDE_LABEL)
        .expect("the start button");
    let asked = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&asked);
    fixture.ctx().set_repaint_observer(move |info| {
        record.lock().expect("the repaint log").push(info.delay);
    });

    // Act: the press is not the click; only the release pass is.
    fixture.press(&start);
    asked.lock().expect("the repaint log").clear();
    fixture.release(&start);
    let delays = asked.lock().expect("the repaint log").clone();
    // egui swaps the cause list at the start of each pass, so the release pass's requests are read
    // after one further frame.
    fixture.idle(1);
    let causes = fixture.ctx().repaint_causes();

    // Assert: the click pass asks for the very next frame, from the app itself.
    //
    // egui already requests an immediate repaint for any pass that carries an input event, so the
    // delay alone cannot attribute the request; the cause names the caller, and the app's own
    // request must be there for the deferred action to be drawn.
    assert!(
        delays.contains(&Duration::ZERO),
        "the click pass asked for {delays:?}"
    );
    assert!(
        causes.iter().any(|cause| cause.file.ends_with("ui/app.rs")),
        "the app did not ask for the next frame itself: {causes:?}"
    );
}

#[test]
fn test_tick_ticks_the_wall_clock_while_the_ride_runs() {
    // Arrange
    let directory = tempfile::tempdir().expect("a temporary directory");
    let ride = loaded_running_ride(&directory).expect("the fixture ride");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let now = Instant::now();

    // Act
    let next = app.borrow_mut().tick(now);

    // Assert: the elapsed and remaining readouts advance once a second.
    assert_eq!(next, Some(now + Duration::from_secs(1)));
}

#[test]
fn test_tick_keeps_the_hourly_deadline_without_a_live_ride() {
    // Arrange
    let (app, _directory) = headless_app(None);
    let now = Instant::now();

    // Act
    let next = app.borrow_mut().tick(now);

    // Assert
    assert_eq!(next, Some(super::next_hour_instant(now)));
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
    assert_eq!(app.borrow().state.setup_edit, None);
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

/// Build a `Ride` reply from *ride*, carrying `audit_row_count`.
fn ride_reply(ride: LoadedRide, audit_row_count: usize) -> StoreReply {
    StoreReply::Ride {
        id: ride.ride_id,
        config: ride.config,
        roster: Box::new(ride.roster),
        engine: Box::new(ride.engine),
        audit_row_count,
    }
}

#[test]
fn test_apply_ride_reply_with_audit_rows_enables_the_audit_trail()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let (app, _app_directory) = headless_app(None);

    // Act
    app.borrow_mut().apply_reply(Ok(ride_reply(ride, 2)));

    // Assert
    assert_eq!(app.borrow().state.audit_row_count, 2);
    assert!(enabled(MI_AUDIT_TRAIL, &app.borrow().state.gate()));
    Ok(())
}

#[test]
fn test_apply_ride_reply_without_audit_rows_keeps_the_audit_trail_disabled()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let (app, _app_directory) = headless_app(None);

    // Act
    app.borrow_mut().apply_reply(Ok(ride_reply(ride, 0)));

    // Assert
    assert_eq!(app.borrow().state.audit_row_count, 0);
    assert!(!enabled(MI_AUDIT_TRAIL, &app.borrow().state.gate()));
    Ok(())
}

#[test]
fn test_opening_the_audit_viewer_after_a_ride_load_shows_the_rows()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange: the ride load reports two audit rows.
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let (app, _app_directory) = headless_app(None);
    app.borrow_mut().apply_reply(Ok(ride_reply(ride, 2)));
    let at = time::OffsetDateTime::from_unix_timestamp(1_700_000_000)?;
    let rows = vec![
        rivercrossing_store::AuditRow {
            at,
            action: "record_crossing".to_owned(),
            entry: PLATE.to_owned(),
            reason: "Ada Lovelace".to_owned(),
        },
        rivercrossing_store::AuditRow {
            at,
            action: "start".to_owned(),
            entry: String::new(),
            reason: "0:00:00".to_owned(),
        },
    ];

    // Act: open the viewer, then answer its row request.
    app.borrow_mut().state.open_audit();
    app.borrow_mut()
        .apply_reply(Ok(StoreReply::AuditRows(rows)));

    // Assert
    let guard = app.borrow();
    assert_eq!(guard.state.audit.store_rows.len(), 2);
    assert_eq!(guard.state.filtered_audit().len(), 2);
    Ok(())
}

#[test]
fn test_apply_audit_rows_with_no_rows_clears_the_count_and_the_viewer()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange: a ride load reports two audit rows.
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let (app, _app_directory) = headless_app(None);
    app.borrow_mut().apply_reply(Ok(ride_reply(ride, 2)));

    // Act
    app.borrow_mut()
        .apply_reply(Ok(StoreReply::AuditRows(Vec::new())));

    // Assert
    assert_eq!(app.borrow().state.audit_row_count, 0);
    assert_eq!(app.borrow().state.audit.store_rows.len(), 0);
    Ok(())
}

#[test]
fn test_apply_ride_reply_given_another_reply_leaves_the_ride_unset() {
    // Arrange
    let (app, _app_directory) = headless_app(None);

    // Act
    app.borrow_mut()
        .apply_ride_reply(StoreReply::Rides(Vec::new()));

    // Assert
    assert!(app.borrow().state.ride.is_none());
}

// ------------------------------------------------------ edit ride

/// Build a headless app holding *ride* over *directory*'s paths.
fn app_with_ride(directory: &TempDir, ride: LoadedRide) -> Rc<RefCell<RiverCrossingApp>> {
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    app
}

#[test]
fn test_the_edit_ride_command_opens_the_live_setup_titled_edit_ride()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange: a draft ride loaded into a headless app.
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let ride_id = ride.ride_id;
    let app = app_with_ride(&directory, ride);

    // Act
    let opened = app.borrow_mut().command("mi_edit_ride")?;

    // Assert
    assert_eq!(opened, serde_json::json!({ "ok": true }));
    assert_eq!(app.borrow().state.setup_edit, Some(ride_id));
    assert_eq!(
        app.borrow().state.dialogs.field("name_input"),
        Some(RIDE_NAME)
    );
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    assert!(tree_mentions(fixture.tree(), "Edit Ride"));
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn test_saving_an_edit_queues_an_update_not_a_create() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange: a draft ride open in the edit setup. Its planned duration is whole hours, so the
    // `H:MM` field round-trips it.
    let directory = tempfile::tempdir()?;
    let mut ride = loaded_draft_ride(&directory)?;
    ride.config.planned_duration_s = 7_200;
    let ride_id = ride.ride_id;
    let app = app_with_ride(&directory, ride);
    app.borrow_mut().command("mi_edit_ride")?;

    // Act
    let result = app.borrow_mut().state.submit_setup();

    // Assert
    assert_eq!(result, Ok(()));
    let guard = app.borrow();
    assert_eq!(guard.state.pending_create, None);
    let updated = guard
        .state
        .pending_store
        .iter()
        .find_map(|command| match command {
            StoreCommand::UpdateRide { id, config } if *id == ride_id => Some(config.name.as_str()),
            _ => None,
        });
    assert_eq!(updated, Some(RIDE_NAME));
    Ok(())
}

#[test]
fn test_the_ride_updated_reply_changes_the_live_config_and_keeps_the_ride_data()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange: a running ride holding one crossing.
    let directory = tempfile::tempdir()?;
    let mut ride = loaded_running_ride(&directory)?;
    let _ = ride.engine.record_crossing(PLATE, None);
    let ride_id = ride.ride_id;
    let crossings = ride.engine.crossings().len();
    let events = ride.engine.events().len();
    let mut config = ride.config.clone();
    config.name = "RENAMED RIDE".to_owned();
    let app = app_with_ride(&directory, ride);

    // Act
    app.borrow_mut().apply_reply(Ok(StoreReply::RideUpdated {
        id: ride_id,
        config,
    }));

    // Assert
    let guard = app.borrow();
    let ride = guard.state.ride.as_ref().ok_or("the ride is loaded")?;
    assert_eq!(ride.config.name, "RENAMED RIDE");
    assert_eq!(ride.engine.config().name, "RENAMED RIDE");
    assert_eq!(ride.engine.crossings().len(), crossings);
    assert_eq!(ride.engine.events().len(), events);
    assert_eq!(guard.state.notice.as_deref(), Some(RIDE_UPDATED));
    assert_eq!(guard.state.dialogs.top(), None);
    Ok(())
}

#[test]
fn test_editing_a_running_ride_leaves_it_running() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange: a live ride that the operator edits.
    let directory = tempfile::tempdir()?;
    let ride = loaded_running_ride(&directory)?;
    let ride_id = ride.ride_id;
    let mut config = ride.config.clone();
    config.lap_km = 9.5;
    let app = app_with_ride(&directory, ride);

    // Act
    app.borrow_mut().apply_reply(Ok(StoreReply::RideUpdated {
        id: ride_id,
        config,
    }));

    // Assert
    let guard = app.borrow();
    let ride = guard.state.ride.as_ref().ok_or("the ride is loaded")?;
    assert_eq!(ride.engine.state(), RideStatus::Running);
    assert_eq!(ride.engine.config().lap_km, 9.5);
    Ok(())
}

#[test]
fn test_a_ride_updated_reply_for_another_ride_is_ignored() -> Result<(), Box<dyn std::error::Error>>
{
    // Arrange: the open ride is id 1; the reply names id 2.
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let ride_id = ride.ride_id;
    let mut config = ride.config.clone();
    config.name = "ANOTHER RIDE".to_owned();
    let app = app_with_ride(&directory, ride);

    // Act
    app.borrow_mut().apply_reply(Ok(StoreReply::RideUpdated {
        id: ride_id + 1,
        config,
    }));

    // Assert
    let guard = app.borrow();
    let ride = guard.state.ride.as_ref().ok_or("the ride is loaded")?;
    assert_eq!(ride.engine.config().name, RIDE_NAME);
    assert_eq!(ride.config.name, RIDE_NAME);
    assert_eq!(guard.state.notice, None);
    Ok(())
}

#[test]
fn test_a_ride_updated_reply_reports_an_engine_refusal() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange: a draft ride whose edited setup names 256 decks, which the shoe cannot build.
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let ride_id = ride.ride_id;
    let mut config = ride.config.clone();
    config.deck_count = 256;
    let app = app_with_ride(&directory, ride);

    // Act
    app.borrow_mut().apply_reply(Ok(StoreReply::RideUpdated {
        id: ride_id,
        config,
    }));

    // Assert
    let guard = app.borrow();
    let ride = guard.state.ride.as_ref().ok_or("the ride is loaded")?;
    assert_eq!(
        guard.state.notice.as_deref(),
        Some("invalid ride config: deck_count must be at most 255, got 256")
    );
    assert_eq!(ride.engine.config().deck_count, 8);
    Ok(())
}

#[test]
fn test_a_ride_updated_reply_with_no_open_ride_lands_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange: no ride is loaded when the reply arrives.
    let (app, _directory) = headless_app(None);
    let config = fixture_config()?;

    // Act
    app.borrow_mut()
        .apply_reply(Ok(StoreReply::RideUpdated { id: 1, config }));

    // Assert
    let guard = app.borrow();
    assert_eq!(guard.state.ride.as_ref().map(|ride| ride.ride_id), None);
    assert_eq!(guard.state.notice, None);
    Ok(())
}

#[test]
fn test_a_ride_updated_reply_keeps_a_later_new_setup_dialog_open()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange: the operator cancelled the edit and opened New Ride before the reply landed.
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let ride_id = ride.ride_id;
    let mut config = ride.config.clone();
    config.name = "EDITED".to_owned();
    let app = app_with_ride(&directory, ride);
    app.borrow_mut()
        .state
        .open_setup_dialog(time::OffsetDateTime::now_utc().date());

    // Act
    app.borrow_mut().apply_reply(Ok(StoreReply::RideUpdated {
        id: ride_id,
        config,
    }));

    // Assert
    let guard = app.borrow();
    assert_eq!(guard.state.setup_edit, None);
    assert_eq!(
        guard.state.dialogs.top().map(|dialog| dialog.id),
        Some("ride_setup_dlg")
    );
    assert_eq!(
        guard
            .state
            .ride
            .as_ref()
            .map(|ride| ride.engine.config().name.clone()),
        Some("EDITED".to_owned())
    );
    Ok(())
}

#[test]
fn test_a_ride_updated_reply_keeps_a_dialog_above_the_edit_setup_open()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange: another dialog sits on top of the edit setup when the reply lands.
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let ride_id = ride.ride_id;
    let mut config = ride.config.clone();
    config.name = "EDITED".to_owned();
    let app = app_with_ride(&directory, ride);
    app.borrow_mut().command("mi_edit_ride")?;
    app.borrow_mut()
        .state
        .dialogs
        .open("about_dlg", None)
        .map_err(|_| "the about dialog refuses to open")?;

    // Act
    app.borrow_mut().apply_reply(Ok(StoreReply::RideUpdated {
        id: ride_id,
        config,
    }));

    // Assert
    let guard = app.borrow();
    assert_eq!(guard.state.setup_edit, None);
    assert_eq!(
        guard.state.dialogs.top().map(|dialog| dialog.id),
        Some("about_dlg")
    );
    Ok(())
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
    let stop = fixture.by_label(STOP_RIDE_LABEL).expect("the stop button");

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

// ---------------------------------------------------------------- editors

/// Return a restored solo entry on *plate*.
#[expect(
    clippy::too_many_arguments,
    reason = "one fixture call spells the entry, its plate and its rider"
)]
fn editor_solo(key: &str, plate: &str, first: &str, last: &str) -> Entry {
    Entry::restored(
        key,
        plate,
        format!("{first} {last}"),
        EntryType::Solo,
        vec![Rider {
            first_name: first.to_owned(),
            last_name: last.to_owned(),
            plate: Some(plate.to_owned()),
            ..Rider::default()
        }],
    )
}

/// Build a DRAFT ride over a roster with a solo pair and a team, from a fresh store in *directory*.
fn loaded_draft_editors(directory: &TempDir) -> Result<LoadedRide, String> {
    let mut store =
        Store::open(directory.path().join("editors.db")).map_err(|error| error.to_string())?;
    let config = fixture_config()?;
    let ride_id = store
        .create_ride(&config, Some(SEED))
        .map_err(|error| error.to_string())?;
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster.load_entries(vec![
        editor_solo("k-ada", "101", "Ada", "Lovelace"),
        editor_solo("k-grace", "102", "Grace", "Hopper"),
        Entry::restored(
            "k-alpha",
            "A1",
            "Alpha",
            EntryType::Team,
            vec![
                Rider {
                    first_name: "Alan".to_owned(),
                    last_name: "Turing".to_owned(),
                    plate: Some("201".to_owned()),
                    ..Rider::default()
                },
                Rider {
                    first_name: "Barbara".to_owned(),
                    last_name: "Liskov".to_owned(),
                    plate: Some("202".to_owned()),
                    ..Rider::default()
                },
            ],
        ),
    ]);
    store
        .save_roster(ride_id, &roster)
        .map_err(|error| error.to_string())?;
    let engine = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;
    let mut roster = store
        .roster_for(ride_id)
        .map_err(|error| error.to_string())?;
    roster.set_status(engine.state());
    Ok(LoadedRide {
        ride_id,
        config: engine.config().clone(),
        roster,
        engine,
    })
}

/// Build a headless app whose draft ride carries the editor roster.
fn app_with_editors(
    directory: &TempDir,
) -> Result<Rc<RefCell<RiverCrossingApp>>, Box<dyn std::error::Error>> {
    let ride = loaded_draft_editors(directory).map_err(std::io::Error::other)?;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    Ok(app)
}

#[test]
fn test_the_rider_editor_opens_from_the_menu() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = app_with_editors(&directory)?;
    let mut fixture = app_fixture(&app);

    // Act
    let opened = app.borrow_mut().command("mi_rider_editor")?;
    fixture.idle(2);

    // Assert
    assert_eq!(opened, serde_json::json!({"ok": true}));
    assert!(tree_mentions(fixture.tree(), "Rider Editor"));
    assert!(tree_mentions(fixture.tree(), "Ada Lovelace"));
    assert!(fixture.by_label("Add rider…").is_some());
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn test_the_rider_editor_add_flow_records_a_rider() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = app_with_editors(&directory)?;
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("mi_rider_editor")?;
    fixture.idle(2);
    let add = fixture.by_label("Add rider…").expect("the add button");

    // Act
    fixture.click(&add);
    fixture.idle(2);
    {
        let mut app = app.borrow_mut();
        app.set_dialog_field("first_name_input", "Katherine")?;
        app.set_dialog_field("last_name_input", "Johnson")?;
    }
    fixture.idle(1);
    let save = fixture.by_label("Save").expect("the Save button");
    fixture.click(&save);
    fixture.idle(2);

    // Assert
    let app_guard = app.borrow();
    assert_eq!(app_guard.state.rider_editor.rows.len(), 5);
    assert_eq!(app_guard.state.notice.as_deref(), Some("Rider added"));
    assert!(tree_mentions(fixture.tree(), "Katherine Johnson"));
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn test_the_teams_editor_and_the_issues_report_open_from_the_menu()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = app_with_editors(&directory)?;
    let mut fixture = app_fixture(&app);

    // Act: the teams editor.
    app.borrow_mut().command("mi_team_editor")?;
    fixture.idle(2);
    let teams_open = tree_mentions(fixture.tree(), "Teams Editor");
    let team_row = tree_mentions(fixture.tree(), "Alpha");
    {
        let mut app = app.borrow_mut();
        app.command("mi_team_editor")?;
    }
    fixture.idle(1);

    // Act: cancel the editor and open the issues report.
    let close = fixture.by_label("Close").expect("the close button");
    fixture.click(&close);
    fixture.idle(1);
    app.borrow_mut().command("mi_check_rider_issues")?;
    fixture.idle(2);

    // Assert
    assert!(teams_open);
    assert!(team_row);
    assert!(tree_mentions(fixture.tree(), "Rider Issues"));
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn test_the_csv_preview_opens_from_the_picker_seam() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = app_with_editors(&directory)?;
    let path = directory.path().join("entries.csv");
    std::fs::write(
        &path,
        "FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\nKatherine,Johnson,solo,,111,,\n",
    )?;

    // Act
    app.borrow_mut().open_csv_preview(&path)?;
    let mut fixture = app_fixture(&app);
    fixture.idle(2);

    // Assert
    assert!(tree_mentions(fixture.tree(), "Import Riders"));
    assert!(tree_mentions(fixture.tree(), "1 riders"));
    let import = fixture.by_label("Import").expect("the Import button");
    assert!(!fixture.tree().is_disabled_key(import.node));
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn test_the_csv_import_row_opens_the_preview_on_the_picked_path()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = app_with_editors(&directory)?;
    let path = directory.path().join("entries.csv");
    std::fs::write(
        &path,
        "FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\nKatherine,Johnson,solo,,111,,\n",
    )?;
    app.borrow_mut()
        .answer_file_pickers_with(vec![path.clone()]);
    app.borrow_mut().command("mi_import_csv")?;
    let mut fixture = app_fixture(&app);

    // Act
    pump(&app, &mut fixture, 3);

    // Assert
    let app_guard = app.borrow();
    let state = &app_guard.state;
    assert_eq!(
        state.dialogs.top().map(|dialog| dialog.id),
        Some("csv_preview_dlg")
    );
    assert_eq!(state.csv_preview.path.as_deref(), Some(path.as_path()));
    assert_eq!(
        state
            .csv_preview
            .model
            .as_ref()
            .map(|model| model.summary.clone()),
        Some("entries.csv → 1 riders · 0 teams · 0 conflicts".to_owned())
    );
    Ok(())
}

#[test]
fn test_the_csv_import_row_without_a_picker_posts_the_notice()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange: the headless host answers every picker as unavailable.
    let directory = tempfile::tempdir()?;
    let app = app_with_editors(&directory)?;
    app.borrow_mut().command("mi_import_csv")?;
    let mut fixture = app_fixture(&app);

    // Act
    pump(&app, &mut fixture, 3);

    // Assert
    assert_eq!(
        app.borrow().state.notice.as_deref(),
        Some(FILE_PICKER_UNAVAILABLE)
    );
    assert_eq!(app.borrow().state.dialogs.top(), None);
    Ok(())
}

#[test]
fn test_the_console_rider_row_opens_the_editor_at_the_plate()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = app_with_editors(&directory)?;
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let row = fixture.by_label("Ada Lovelace").expect("the rider row");

    // Act
    fixture.click(&row);
    fixture.idle(2);

    // Assert
    let app_guard = app.borrow();
    assert_eq!(
        app_guard.state.dialogs.top().map(|dialog| dialog.id),
        Some("rider_editor_dlg")
    );
    assert_eq!(
        app_guard
            .state
            .rider_editor
            .selected
            .as_ref()
            .map(|selection| selection.entry.0.clone()),
        Some("k-ada".to_owned())
    );
    let inspected = crate::ui::automation::actions::inspect(
        &app_guard.state,
        &serde_json::json!({"field": "plate_input"}),
    );
    assert_eq!(inspected, Ok(serde_json::json!({"value": "101"})));
    Ok(())
}

// ------------------------------------------------- lifecycle & system

use rivercrossing_store::RideRow;
use time::Month;

/// Return a stored ride row.
fn app_ride_row(id: i64, name: &str, status: rivercrossing_ride::RideStatus) -> RideRow {
    RideRow {
        id,
        name: name.to_owned(),
        event_date: time::Date::from_calendar_date(2026, Month::September, 20)
            .expect("the fixture date"),
        status,
        entries: 1,
    }
}

/// Run the app's logic pass over *fixture*'s context.
fn run_logic(app: &Rc<RefCell<RiverCrossingApp>>, fixture: &Fixture) {
    let ctx = fixture.ctx().clone();
    app.borrow_mut().logic(&ctx);
}

/// Run *passes* ui-and-logic passes over *fixture*, so a native answer reaches the logic pass.
fn pump(app: &Rc<RefCell<RiverCrossingApp>>, fixture: &mut Fixture, passes: usize) {
    for _ in 0..passes {
        fixture.idle(1);
        run_logic(app, fixture);
    }
}

#[test]
fn test_a_native_dialog_falls_back_to_the_in_app_modal() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let mut ride = loaded_running_ride(&directory)?;
    let _ = ride.engine.record_crossing(PLATE, None);
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let opened = app.borrow_mut().command("mi_undo_crossing")?;
    assert_eq!(opened, serde_json::json!({"ok": true}));
    assert!(
        app.borrow().state.pending_native.is_some(),
        "the action queued the confirm"
    );
    let mut fixture = app_fixture(&app);

    // Act: raise the native box, let the headless host answer unavailable, then render.
    run_logic(&app, &fixture);
    assert!(
        app.borrow().state.pending_native.is_none(),
        "the first pass raised the native box"
    );
    fixture.idle(1);
    run_logic(&app, &fixture);
    assert!(
        app.borrow().state.pending_native.is_some(),
        "the unavailable host restored the in-app modal"
    );
    fixture.idle(2);

    // Assert: the in-app modal is showing and cancelling keeps the crossing.
    assert!(tree_mentions(fixture.tree(), "Undo the last crossing?"));
    let cancel = fixture.by_label("Cancel").expect("the cancel button");
    fixture.click(&cancel);
    fixture.idle(1);
    let app_guard = app.borrow();
    assert_eq!(app_guard.state.pending_native, None);
    assert_eq!(
        app_guard
            .state
            .ride
            .as_ref()
            .map(|ride| ride.engine.crossings().len()),
        Some(1)
    );
    Ok(())
}

/// How the quit confirm is raised: the root window's close button, or File ▸ Exit.
#[derive(Clone, Copy, Debug)]
enum QuitRoute {
    /// The root window's close button.
    Close,
    /// The Exit row, which is ⌘Q in the macOS app menu.
    Menu,
}

/// Raise the quit confirm through *route* and return the close decision it reports.
fn raise_quit(
    app: &Rc<RefCell<RiverCrossingApp>>,
    route: QuitRoute,
) -> Result<CloseDecision, String> {
    match route {
        QuitRoute::Close => Ok(app.borrow_mut().close_requested()),
        QuitRoute::Menu => app
            .borrow_mut()
            .command("wxID_EXIT")
            .map(|_| CloseDecision::Hold),
    }
}

/// Return how a quit is raised on *os*: the Exit menu row is macOS's one quit entrance.
///
/// macOS hides the window on its close button, so its quit confirm is reachable only from
/// File ▸ Exit (⌘Q); Windows and Linux raise it from the close button itself.
fn quit_route(os: Os) -> QuitRoute {
    if os == Os::Mac {
        QuitRoute::Menu
    } else {
        QuitRoute::Close
    }
}

#[rstest]
#[case::this_platform(Os::current())]
fn test_the_quit_confirm_closes_the_app(#[case] os: Os) -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    let held = raise_quit(&app, quit_route(os))?;
    assert_eq!(held, CloseDecision::Hold);
    assert!(
        app.borrow().state.pending_native.is_some(),
        "the quit confirm is raised"
    );

    // Act: the headless fallback modal shows the plain confirm, and Quit closes.
    run_logic(&app, &fixture);
    fixture.idle(1);
    run_logic(&app, &fixture);
    fixture.idle(2);
    assert!(tree_mentions(fixture.tree(), "Quit RiverCrossing?"));
    let quit = fixture.by_label("Confirm Quit").expect("the quit button");
    fixture.click(&quit);
    fixture.idle(1);
    run_logic(&app, &fixture);

    // Assert
    assert!(app.borrow().quit_confirmed);
    Ok(())
}

/// Pick the appearance entry labelled *entry* in the open Settings dialog.
fn pick_appearance(fixture: &mut Fixture, entry: &str) {
    let picker = fixture.by_label("Theme").expect("the theme picker");
    fixture.click(&picker);
    fixture.idle(2);
    let row = fixture.by_label(entry).expect("the appearance entry shows");
    fixture.click(&row);
    fixture.idle(2);
}

#[test]
fn test_the_settings_dialog_shows_the_theme_picker_and_no_mode_radios()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);

    // Act
    let picker = fixture
        .by_label("Theme")
        .map(|target| fixture.tree().role_of(target.node));
    let radios = fixture.by_role(Role::RADIO_BUTTON).len();
    let groups = fixture.by_role(Role::RADIO_GROUP).len();

    // Assert: the combined picker owns the appearance, and the Mode radio group is gone.
    assert_eq!(picker, Some(Some(Role::COMBO_BOX)));
    assert_eq!((radios, groups), (0, 0));
    Ok(())
}

#[test]
fn test_the_settings_dialog_renders_the_appearance_axes() -> Result<(), Box<dyn std::error::Error>>
{
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);

    // Act
    let rendered = ["Contrast", "Density", "Text size", "Theme"]
        .map(|label| fixture.by_label(label).is_some());
    let current = fixture
        .by_label("Theme")
        .and_then(|target| fixture.tree().value_of(target.node))
        .map(str::to_owned);

    // Assert: the picker opens on the stored pair's own entry.
    assert_eq!(rendered, [true, true, true, true]);
    assert_eq!(current.as_deref(), Some("ArtCraft — System"));
    Ok(())
}

#[test]
fn test_picking_a_graphite_dark_appearance_sets_the_theme_and_the_dark_appearance()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);

    // Act
    pick_appearance(&mut fixture, "Graphite — Dark");

    // Assert
    let form = app
        .borrow()
        .state
        .settings_form
        .clone()
        .expect("the settings form stays open");
    assert_eq!(
        (form.theme.as_str(), form.appearance.as_str()),
        ("craft.graphite", "dark")
    );
    Ok(())
}

#[test]
fn test_picking_a_dark_only_appearance_sets_the_dark_appearance()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);

    // Act: Solarized Dark sits below the popup's first screen, so pick it by its entry index.
    let picker = fixture.by_label("Theme").expect("the theme picker");
    let entry = settings_form::appearance_entries()
        .iter()
        .position(|(label, _)| label == "Solarized Dark")
        .expect("the dark-only entry");
    let index = f64::from(u32::try_from(entry).expect("the entry index fits a u32"));
    fixture.access(AccessActionKind::SetValue(index), &picker);
    fixture.idle(2);

    // Assert
    let form = app
        .borrow()
        .state
        .settings_form
        .clone()
        .expect("the settings form stays open");
    assert_eq!(
        (form.theme.as_str(), form.appearance.as_str()),
        ("craft.solarized-dark", "dark")
    );
    Ok(())
}

#[test]
fn test_picking_a_both_themes_system_entry_sets_the_system_appearance()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);

    // Act: move off the stored pair first, then pick the system entry to prove it writes back.
    pick_appearance(&mut fixture, "Graphite — Dark");
    pick_appearance(&mut fixture, "ArtCraft — System");

    // Assert
    let form = app
        .borrow()
        .state
        .settings_form
        .clone()
        .expect("the settings form stays open");
    assert_eq!(
        (form.theme.as_str(), form.appearance.as_str()),
        ("craft.artcraft", "system")
    );
    Ok(())
}

#[test]
fn test_opening_settings_with_an_unsupported_variant_keeps_the_pair_and_shows_the_first_entry()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange: Classic is light-only, so the stored System appearance names no entry.
    let (app, _directory) = headless_app(None);
    {
        let mut app = app.borrow_mut();
        app.state.settings.theme = "craft.classic".to_owned();
        app.state.settings.appearance = "system".to_owned();
    }
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);

    // Act
    let form = app
        .borrow()
        .state
        .settings_form
        .clone()
        .expect("the settings form stays open");
    let shown = fixture
        .by_label("Theme")
        .and_then(|target| fixture.tree().value_of(target.node))
        .map(str::to_owned);

    // Assert: the picker shows the theme's first entry, and the form keeps the stored pair.
    assert_eq!(
        (form.theme.as_str(), form.appearance.as_str()),
        ("craft.classic", "system")
    );
    assert_eq!(shown.as_deref(), Some("Classic"));
    Ok(())
}

#[test]
fn test_the_settings_rows_put_each_caption_beside_its_control()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);

    // Act: a caption on the control's own row shares its vertical band.
    let rows = ["Theme", "Contrast", "Density", "Text size"].map(|label| {
        let caption = fixture
            .by_role(Role::LABEL)
            .into_iter()
            .find(|target| fixture.tree().name_of(target.node) == Some(label))
            .expect("the caption shows");
        let control = fixture.by_label(label).expect("the control shows");
        let middle = caption.rect.center().y;
        middle > control.rect.min.y && middle < control.rect.max.y
    });

    // Assert
    assert_eq!(rows, [true, true, true, true]);
    Ok(())
}

#[test]
fn test_the_settings_dialog_footer_reads_ok_and_not_save() -> Result<(), Box<dyn std::error::Error>>
{
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);

    // Act
    let ok = fixture.by_label("OK").is_some();
    let save = fixture.by_label("Save").is_some();

    // Assert
    assert_eq!((ok, save), (true, false));
    Ok(())
}

#[test]
fn test_the_settings_columns_share_a_row() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);

    // Act: Contrast and Density stand in the design's two-column row.
    let contrast = fixture.by_label("Contrast").expect("the contrast picker");
    let density = fixture.by_label("Density").expect("the density picker");

    // Assert: the two captions share a row, and the second stands in the right-hand column.
    let shared =
        contrast.rect.min.y < density.rect.max.y && density.rect.min.y < contrast.rect.max.y;
    assert!(
        shared,
        "the two pickers share one row: {:?} {:?}",
        contrast.rect, density.rect
    );
    assert!(
        density.rect.min.x > contrast.rect.min.x,
        "the density column stands right of contrast: {:?} {:?}",
        contrast.rect,
        density.rect
    );
    Ok(())
}

#[test]
fn test_the_settings_notice_sits_over_the_footer() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);
    let message = "Average speed is out of range";
    app.borrow_mut().state.settings_errors = Some(vec![message.to_owned()]);
    fixture.idle(2);

    // Act
    let verbose = fixture
        .by_label("Verbose logging")
        .expect("the last console check");
    let notice = fixture
        .tree()
        .keys()
        .into_iter()
        .find(|key| {
            fixture
                .tree()
                .name_of(*key)
                .is_some_and(|name| name.contains(message))
        })
        .and_then(|key| fixture.target(key))
        .expect("the refusal notice shows");
    let ok = fixture.by_label("OK").expect("the footer OK");

    // Assert: the notice follows the checks and precedes the footer.
    assert!(
        notice.rect.center().y > verbose.rect.center().y,
        "the notice follows the console checks: {:?} {:?}",
        notice.rect,
        verbose.rect
    );
    assert!(
        notice.rect.max.y <= ok.rect.min.y,
        "the notice stands directly over the footer: {:?} {:?}",
        notice.rect,
        ok.rect
    );
    Ok(())
}

#[test]
fn test_picking_a_contrast_entry_writes_the_form() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);

    // Act: the horizontal axis row still writes the picked choice back.
    let contrast = fixture.by_label("Contrast").expect("the contrast picker");
    fixture.click(&contrast);
    fixture.idle(2);
    let high = fixture.by_label("High").expect("the High entry");
    fixture.click(&high);
    fixture.idle(2);

    // Assert
    let form = app
        .borrow()
        .state
        .settings_form
        .clone()
        .expect("the settings form stays open");
    assert_eq!(form.contrast, "high");
    Ok(())
}

#[test]
fn test_the_settings_dialog_applies_through_the_view() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let (app, _directory) = headless_app(None);
    let mut fixture = app_fixture(&app);
    app.borrow_mut().command("wxID_PREFERENCES")?;
    fixture.idle(2);

    // Act
    pick_appearance(&mut fixture, "ArtCraft — Dark");
    let sound = fixture
        .by_label("Sound on crossing")
        .expect("the sound checkbox");
    fixture.click(&sound);
    fixture.idle(1);
    let ok = fixture.by_label("OK").expect("the OK button");
    fixture.click(&ok);
    fixture.idle(2);

    // Assert
    let app_guard = app.borrow();
    assert_eq!(app_guard.state.settings.appearance, "dark");
    assert_eq!(app_guard.state.settings.theme, "craft.artcraft");
    assert!(!app_guard.state.settings.sound_on);
    assert_eq!(app_guard.state.dialogs.top(), None);
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn test_the_library_dialog_renders_its_rows() {
    // Arrange
    let (app, _directory) = headless_app(None);
    app.borrow_mut().show_library(vec![
        app_ride_row(
            1,
            "GORBA EPIC 2026",
            rivercrossing_ride::RideStatus::Running,
        ),
        app_ride_row(
            2,
            "Club poker night",
            rivercrossing_ride::RideStatus::Finished,
        ),
    ]);
    let mut fixture = app_fixture(&app);

    // Act
    fixture.idle(2);

    // Assert
    assert_eq!(
        app.borrow().state.dialogs.top().map(|dialog| dialog.id),
        Some("ride_library_dlg")
    );
    assert!(tree_mentions(fixture.tree(), "GORBA EPIC 2026"));
    assert!(tree_mentions(fixture.tree(), "FINISHED"));
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
}

#[test]
fn test_the_delete_ride_dialog_requires_the_typed_name() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let (app, _directory) = headless_app(None);
    app.borrow_mut().show_delete_ride(app_ride_row(
        2,
        "Club poker night",
        rivercrossing_ride::RideStatus::Finished,
    ));
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let delete = fixture.by_label("Delete").expect("the delete button");
    let disabled_before = fixture.tree().is_disabled_key(delete.node);

    // Act
    app.borrow_mut()
        .set_dialog_field("confirm_name_input", "Club poker night")?;
    fixture.idle(2);
    let delete = fixture.by_label("Delete").expect("the delete button");

    // Assert
    assert!(
        disabled_before,
        "the delete is armed only by the typed name"
    );
    assert!(!fixture.tree().is_disabled_key(delete.node));
    Ok(())
}

#[test]
fn test_the_audit_and_selftest_dialogs_render() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_editors(&directory)?;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let at = time::OffsetDateTime::from_unix_timestamp(1_700_000_000)?;
    app.borrow_mut()
        .show_audit(vec![rivercrossing_store::AuditRow {
            at,
            action: "record_crossing".to_owned(),
            entry: "101".to_owned(),
            reason: "Ada Lovelace".to_owned(),
        }]);
    let mut fixture = app_fixture(&app);

    // Act
    fixture.idle(2);
    let audit_open = tree_mentions(fixture.tree(), "Audit Trail");
    let close = fixture.by_label("Close").expect("the close button");
    fixture.click(&close);
    fixture.idle(1);
    app.borrow_mut().command("mi_selftest")?;
    fixture.idle(2);

    // Assert
    assert!(audit_open);
    assert!(tree_mentions(fixture.tree(), "Evaluator Self-Test"));
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn test_the_crossing_detail_renders_from_the_feed() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let mut ride = loaded_running_ride(&directory)?;
    let _ = ride.engine.record_crossing(PLATE, None);
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);

    // Act
    let opened = app.borrow_mut().open_first_crossing_detail();
    let mut fixture = app_fixture(&app);
    fixture.idle(2);

    // Assert
    assert!(opened);
    assert!(tree_mentions(fixture.tree(), "Crossing Detail"));
    assert!(fixture.by_label("Edit Plate").is_some());
    assert_eq!(
        violations(fixture.ctx(), fixture.tree()),
        Vec::<String>::new()
    );
    Ok(())
}

// -------------------------------------------------- results, exports, jobs

#[test]
fn test_open_notice_given_a_failing_opener_names_the_target() {
    // Act
    let notice = super::open_notice("/tmp/results.html", &|_target| Err("no handler".to_owned()));

    // Assert
    assert_eq!(
        notice,
        Some("cannot open /tmp/results.html: no handler".to_owned())
    );
}

#[test]
fn test_open_notice_given_a_working_opener_is_silent() {
    // Act
    let notice = super::open_notice("/tmp/results.html", &|_target| Ok(()));

    // Assert
    assert_eq!(notice, None);
}

#[test]
fn test_an_export_picker_answer_queues_the_worker_job() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let mut ride = loaded_running_ride(&directory)?;
    ride.engine.finish(&[])?;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let target = directory.path().join("results.html");
    app.borrow_mut()
        .answer_file_pickers_with(vec![target.clone()]);
    app.borrow_mut()
        .raise_export_picker(crate::ui::state::exports::ExportKind::Html);
    let mut fixture = app_fixture(&app);

    // Act: the ui pass shows the picker, the logic passes queue and run the job.
    for _ in 0..30 {
        fixture.idle(1);
        run_logic(&app, &fixture);
    }

    // Assert
    assert!(target.exists());
    assert!(app.borrow().state.exports.html);
    assert_eq!(
        app.borrow().state.exports.last_html.as_deref(),
        Some(target.as_path())
    );
    Ok(())
}

#[test]
fn test_the_export_roster_command_asks_for_a_csv_path() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let seen: Rc<RefCell<Vec<Request>>> = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&seen);
    app.borrow_mut()
        .native
        .set_override(Some(Box::new(move |request| {
            recorded.borrow_mut().push(request.clone());
            Answer::Cancelled
        })));
    app.borrow_mut().state.pending_command = Some(CommandId::ExportRosterCsv);
    let mut fixture = app_fixture(&app);

    // Act: the command raises the picker, and the ui pass shows it.
    app.borrow_mut().run_pending_command();
    pump(&app, &mut fixture, 1);

    // Assert
    assert_eq!(
        seen.borrow().first(),
        Some(&Request::File(
            FileDialog::save()
                .title("Save roster CSV")
                .filter("CSV", &["csv"])
                .file_name("gorba-epic-2026-roster.csv")
        ))
    );
    Ok(())
}

#[test]
fn test_the_export_roster_command_without_a_ride_suggests_the_plain_name() {
    // Arrange: no ride is open, so the picker falls back to the plain name.
    let (app, _directory) = headless_app(None);
    let seen: Rc<RefCell<Vec<Request>>> = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&seen);
    app.borrow_mut()
        .native
        .set_override(Some(Box::new(move |request| {
            recorded.borrow_mut().push(request.clone());
            Answer::Cancelled
        })));
    app.borrow_mut().state.pending_command = Some(CommandId::ExportRosterCsv);
    let mut fixture = app_fixture(&app);

    // Act
    app.borrow_mut().run_pending_command();
    pump(&app, &mut fixture, 1);

    // Assert
    assert_eq!(
        seen.borrow().first(),
        Some(&Request::File(
            FileDialog::save()
                .title("Save roster CSV")
                .filter("CSV", &["csv"])
                .file_name("roster.csv")
        ))
    );
}

#[test]
fn test_the_roster_picker_answer_writes_the_csv_and_posts_the_notice()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    let path = directory.path().join("roster.csv");
    app.borrow_mut()
        .answer_file_pickers_with(vec![path.clone()]);
    app.borrow_mut().state.pending_command = Some(CommandId::ExportRosterCsv);
    let mut fixture = app_fixture(&app);

    // Act
    app.borrow_mut().run_pending_command();
    pump(&app, &mut fixture, 2);

    // Assert
    assert!(path.exists());
    assert_eq!(
        app.borrow().state.notice,
        Some(format!("Exported {}", path.display()))
    );
    Ok(())
}

#[test]
fn test_the_export_roster_command_without_a_picker_posts_the_notice()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange: the headless host answers the picker as unavailable.
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    app.borrow_mut().state.pending_command = Some(CommandId::ExportRosterCsv);
    let mut fixture = app_fixture(&app);

    // Act
    app.borrow_mut().run_pending_command();
    pump(&app, &mut fixture, 2);

    // Assert
    assert_eq!(
        app.borrow().state.notice.as_deref(),
        Some(FILE_PICKER_UNAVAILABLE)
    );
    assert_eq!(app.borrow().state.dialogs.top(), None);
    Ok(())
}

#[test]
fn test_the_roster_answer_without_a_ride_posts_the_refusal() {
    // Arrange
    let (app, directory) = headless_app(None);
    let path = directory.path().join("roster.csv");

    // Act
    app.borrow_mut().pick_roster(&[path]);

    // Assert
    assert_eq!(
        app.borrow().state.notice,
        Some("Open a ride first".to_owned())
    );
}

#[test]
fn test_the_csv_answer_given_an_unreadable_file_posts_the_notice()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = app_with_editors(&directory)?;
    let missing = directory.path().join("missing.csv");

    // Act
    app.borrow_mut().pick_csv(&[missing]);

    // Assert
    let notice = app.borrow().state.notice.clone().unwrap_or_default();
    assert!(
        notice.starts_with("Could not read missing.csv: "),
        "unexpected notice: {notice}"
    );
    assert_eq!(app.borrow().state.dialogs.top(), None);
    Ok(())
}

#[test]
fn test_the_csv_answer_without_a_path_does_nothing() {
    // Arrange
    let (app, _directory) = headless_app(None);

    // Act
    app.borrow_mut().pick_csv(&[]);

    // Assert
    assert_eq!(app.borrow().state.notice, None);
    assert_eq!(app.borrow().state.dialogs.top(), None);
}

#[test]
fn test_the_roster_answer_without_a_path_does_nothing() {
    // Arrange
    let (app, _directory) = headless_app(None);

    // Act
    app.borrow_mut().pick_roster(&[]);

    // Assert
    assert_eq!(app.borrow().state.notice, None);
}

#[test]
fn test_preview_given_no_export_posts_the_notice() {
    // Arrange
    let (app, _directory) = headless_app(None);

    // Act
    app.borrow_mut()
        .preview(crate::ui::state::exports::ExportKind::Html);

    // Assert
    assert_eq!(
        app.borrow().state.notice,
        Some("Export the file first".to_owned())
    );
}

// ------------------------------------------------------ R-73 route × state

/// Build a fresh state in *phase*, with one plate crossing where the phase allows one.
fn sweep_state(phase: RidePhase, path: &std::path::Path) -> Result<AppState, String> {
    let mut state = AppState::blank();
    state.audit_row_count = 1;
    state.settings.publish_time_board = true;
    state.exports = ExportPresence {
        html: true,
        poster_html: true,
        pdf: true,
        last_html: Some(std::path::PathBuf::from("/tmp/results.html")),
        last_poster_html: Some(std::path::PathBuf::from("/tmp/poster.html")),
        last_pdf: Some(std::path::PathBuf::from("/tmp/results.pdf")),
    };
    if phase == RidePhase::None {
        return Ok(state);
    }
    let mut store = Store::open(path).map_err(|error| error.to_string())?;
    let id = store
        .create_ride(&fixture_config()?, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(id, &fixture_roster()?)
        .map_err(|error| error.to_string())?;
    let mut engine: RideEngine<Roster, SystemClock> =
        store.load_engine(id).map_err(|error| error.to_string())?;
    if phase != RidePhase::Draft {
        engine.start(None).map_err(|error| error.to_string())?;
        let _ = engine.record_crossing(PLATE, None);
    }
    if matches!(
        phase,
        RidePhase::RunningStopped | RidePhase::Finished | RidePhase::Reopened
    ) {
        engine.stop().map_err(|error| error.to_string())?;
    }
    if matches!(phase, RidePhase::Finished | RidePhase::Reopened) {
        engine.finish(&[]).map_err(|error| error.to_string())?;
    }
    if phase == RidePhase::Reopened {
        engine.reopen().map_err(|error| error.to_string())?;
    }
    let mut roster = store.roster_for(id).map_err(|error| error.to_string())?;
    roster.set_status(engine.state());
    state.ride = Some(LoadedRide {
        ride_id: id,
        config: engine.config().clone(),
        roster,
        engine,
    });
    Ok(state)
}

#[test]
fn test_every_action_is_drivable_or_refused_in_every_ride_state() {
    // Arrange
    let directory = tempfile::tempdir().expect("a temporary directory");
    let phases = [
        RidePhase::None,
        RidePhase::Draft,
        RidePhase::Running,
        RidePhase::RunningStopped,
        RidePhase::Finished,
        RidePhase::Reopened,
    ];

    // Act and assert: one fresh state per action, so no action sees another's side effect.
    for (index, phase) in phases.into_iter().enumerate() {
        let path = directory.path().join(format!("sweep-{index}.db"));
        for row in ACTIONS {
            let mut state = sweep_state(phase, &path)
                .unwrap_or_else(|error| panic!("{phase:?} fixture: {error}"));
            let expected = enabled(row.id, &state.gate());
            let result = state.act(row.id);
            if expected {
                assert!(
                    result.is_ok(),
                    "action {} ({}) is enabled in {phase:?} but returned {result:?}",
                    row.id.as_str(),
                    row.label
                );
            } else {
                assert_eq!(
                    result,
                    Err(DispatchError::Disabled(row.id)),
                    "action {} ({}) is disabled in {phase:?}",
                    row.id.as_str(),
                    row.label
                );
            }
        }
    }
}

// --------------------------------------------------- the simulator dialog

/// The simulator's New Ride button label.
const NEW_RIDE_LABEL: &str = "New Ride";

/// The simulator's GO button label.
const GO_LABEL: &str = "GO";

#[test]
fn test_the_simulation_dialog_offers_new_ride_and_disables_go_with_no_ride() {
    // Arrange
    let (app, _directory) = headless_app(None);
    app.borrow_mut()
        .command("mi_simulation")
        .expect("the simulator dialog");
    let mut fixture = app_fixture(&app);

    // Act
    fixture.idle(2);

    // Assert: with no ride, New Ride is the race row's only live action and GO waits for a ride.
    let tree = fixture.tree();
    let new_ride = fixture
        .by_label(NEW_RIDE_LABEL)
        .expect("the new ride button");
    let go = fixture.by_label(GO_LABEL).expect("the go button");
    assert!(
        !tree.is_disabled_key(new_ride.node),
        "New Ride is live with no ride"
    );
    assert!(tree.is_disabled_key(go.node), "GO needs a ride");
}

#[test]
fn test_the_simulation_dialog_disables_new_ride_and_enables_go_with_a_ride() {
    // Arrange
    let directory = tempfile::tempdir().expect("a temporary directory");
    let ride = loaded_draft_ride(&directory).expect("the fixture ride");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    app.borrow_mut()
        .command("mi_simulation")
        .expect("the simulator dialog");
    let mut fixture = app_fixture(&app);

    // Act
    fixture.idle(2);

    // Assert: with a ride open there is nothing to create, and the race control is live.
    let tree = fixture.tree();
    let new_ride = fixture
        .by_label(NEW_RIDE_LABEL)
        .expect("the new ride button");
    let go = fixture.by_label(GO_LABEL).expect("the go button");
    assert!(
        tree.is_disabled_key(new_ride.node),
        "New Ride needs no ride"
    );
    assert!(!tree.is_disabled_key(go.node), "GO has a ride");
}

#[test]
fn test_the_simulation_new_ride_button_creates_the_test_ride_and_closes() {
    // Arrange
    let (app, _directory) = headless_app(None);
    app.borrow_mut()
        .command("mi_simulation")
        .expect("the simulator dialog");
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let new_ride = fixture
        .by_label(NEW_RIDE_LABEL)
        .expect("the new ride button");

    // Act: the dialog host creates the test ride and closes the simulator.
    fixture.click(&new_ride);

    // Assert
    let app_guard = app.borrow();
    assert!(app_guard.state.dialogs.is_empty());
    let created = app_guard
        .state
        .pending_create
        .as_ref()
        .map(|config| config.name.clone());
    assert_eq!(created, Some("GORBA Test Ride #1".to_owned()));
}

#[test]
fn test_the_simulation_enter_key_submits_the_go_button() {
    // Arrange: a draft ride, so GO is the dialog default and the run validates.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let ride = loaded_draft_ride(&directory).expect("the fixture ride");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    app.borrow_mut()
        .command("mi_simulation")
        .expect("the simulator dialog");
    let mut fixture = app_fixture(&app);
    fixture.idle(2);

    // Act
    fixture.key(EguiKey::Enter, Modifiers::NONE);

    // Assert
    assert_eq!(
        app.borrow().state.dialogs.top().map(|dialog| dialog.id),
        Some("sim_running_dlg")
    );
}

#[test]
fn test_the_simulation_go_button_starts_the_simulation() {
    // Arrange: a draft ride, so GO is live and the run validates.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let ride = loaded_draft_ride(&directory).expect("the fixture ride");
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    app.borrow_mut()
        .command("mi_simulation")
        .expect("the simulator dialog");
    let mut fixture = app_fixture(&app);
    fixture.idle(2);
    let go = fixture.by_label(GO_LABEL).expect("the go button");

    // Act: the dialog host submits the race settings.
    fixture.click(&go);

    // Assert
    assert_eq!(
        app.borrow().state.dialogs.top().map(|dialog| dialog.id),
        Some("sim_running_dlg")
    );
    let laps = app.borrow().state.pending_sim.map(|sim| sim.settings.laps);
    assert_eq!(laps, Some(1));
}

// ------------------------------------------------- windows and the guide

/// The one §15 window row that asks the platform for a file instead of opening a dialog.
const PICKER_WINDOW: &str = "csv_preview_dlg";

/// Assert that a window row's dispatch landed: a dialog opened, or the picker row asked.
fn assert_window_row_landed(window: &str, state: &AppState) {
    if window == PICKER_WINDOW {
        assert!(
            state.choose_csv,
            "the import row did not ask for the picker"
        );
        assert_eq!(state.notice, None);
        assert_eq!(state.dialogs.top(), None);
    } else {
        assert!(
            state.dialogs.top().is_some(),
            "{window} left the dialog stack empty"
        );
    }
}

#[test]
fn test_every_window_target_opens_a_dialog_in_a_state_that_enables_it() -> Result<(), String> {
    // Arrange
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let phases = [
        RidePhase::None,
        RidePhase::Draft,
        RidePhase::Running,
        RidePhase::RunningStopped,
        RidePhase::Finished,
        RidePhase::Reopened,
    ];

    // Act and assert: each window row is dispatched in the first state its gate allows, so a
    // row whose arm opens nothing is caught here rather than by an operator mid-ride.
    for (index, row) in ACTIONS.iter().enumerate() {
        let SpecTarget::Window(window) = row.target else {
            continue;
        };
        let path = directory.path().join(format!("window-{index}.db"));
        let mut state = phases
            .iter()
            .copied()
            .map(|phase| {
                sweep_state(phase, &path)
                    .unwrap_or_else(|error| panic!("{phase:?} fixture: {error}"))
            })
            .find(|state| enabled(row.id, &state.gate()))
            .ok_or_else(|| format!("{} is enabled in no ride state", row.id.as_str()))?;
        let result = state.act(row.id);

        // Assert
        assert_eq!(
            result,
            Ok(Effect::OpenWindow(window)),
            "{} ({}) did not open {window}",
            row.id.as_str(),
            row.label
        );
        assert_window_row_landed(window, &state);
    }
    Ok(())
}

#[test]
fn test_the_duplicate_ride_command_asks_for_the_open_ride() -> Result<(), Box<dyn std::error::Error>>
{
    // Arrange
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let id = ride.ride_id;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    app.borrow_mut().state.pending_command = Some(CommandId::DuplicateRide);

    // Act
    app.borrow_mut().run_pending_command();

    // Assert
    assert_eq!(
        app.borrow().state.pending_native,
        Some(PendingNative {
            dialog: NativeDialog::Prompt,
            then: NativeTask::DuplicateRide { id },
        })
    );
    Ok(())
}

#[test]
fn test_the_duplicate_ride_answer_queues_the_store_command()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange: the command raised its prompt on the open ride.
    let directory = tempfile::tempdir()?;
    let ride = loaded_draft_ride(&directory)?;
    let id = ride.ride_id;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.ride = Some(ride);
    app.borrow_mut().state.pending_command = Some(CommandId::DuplicateRide);
    app.borrow_mut().run_pending_command();

    // Act
    let confirmed = app.borrow_mut().state.confirm_native();

    // Assert
    assert!(confirmed);
    assert!(matches!(
        app.borrow().state.pending_store.last(),
        Some(StoreCommand::DuplicateRide { id: queued }) if *queued == id
    ));
    Ok(())
}

#[test]
fn test_the_user_guide_command_writes_the_page_and_launches_it()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    let expected = directory.path().join(USER_GUIDE_FILE);
    let mut launcher = MockBrowserLauncher::new();
    launcher
        .expect_open()
        .withf(move |path: &Path| path == expected.as_path())
        .times(1)
        .returning(|_| Ok(()));
    app.borrow_mut().set_browser(Box::new(launcher));
    app.borrow_mut().state.pending_command = Some(CommandId::OpenUserGuide);

    // Act
    app.borrow_mut().run_pending_command();

    // Assert
    let written = std::fs::read(directory.path().join(USER_GUIDE_FILE))?;
    assert_eq!(written, USER_GUIDE.as_bytes());
    assert_eq!(app.borrow().state.notice, None);
    Ok(())
}

#[test]
fn test_the_user_guide_launch_failure_posts_a_notice() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = tempfile::tempdir()?;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    let mut launcher = MockBrowserLauncher::new();
    launcher
        .expect_open()
        .withf(|path: &Path| path.ends_with(USER_GUIDE_FILE))
        .times(1)
        .returning(|_| Err("no browser".to_owned()));
    app.borrow_mut().set_browser(Box::new(launcher));
    app.borrow_mut().state.pending_command = Some(CommandId::OpenUserGuide);

    // Act
    app.borrow_mut().run_pending_command();

    // Assert
    assert_eq!(
        app.borrow().state.notice,
        Some("cannot open the user guide: no browser".to_owned())
    );
    Ok(())
}

#[test]
fn test_the_user_guide_write_failure_posts_a_notice() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange: the config directory's name is held by a file, so the guide cannot be created. The
    // launcher has no expectation, so a launch would also fail the test.
    let directory = tempfile::tempdir()?;
    let blocked = directory.path().join("config");
    std::fs::write(&blocked, "not a directory")?;
    let app = Rc::new(RefCell::new(RiverCrossingApp::headless(
        None,
        None,
        paths_in(&directory),
    )));
    app.borrow_mut().state.settings_path = blocked.join("settings.json");
    app.borrow_mut()
        .set_browser(Box::new(MockBrowserLauncher::new()));
    app.borrow_mut().state.pending_command = Some(CommandId::OpenUserGuide);

    // Act
    app.borrow_mut().run_pending_command();

    // Assert
    let notice = app.borrow().state.notice.clone().unwrap_or_default();
    let expected_prefix = format!("cannot create {}", blocked.display());
    assert!(
        notice.starts_with(&expected_prefix),
        "unexpected notice: {notice}"
    );
    Ok(())
}
