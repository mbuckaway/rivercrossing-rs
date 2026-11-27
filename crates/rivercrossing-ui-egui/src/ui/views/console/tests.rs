// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the console: the rail's gate hints and hover, and the ride header's statistics.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use craft::egui::{Color32, Event, Id, Rect, Shape, Ui, Vec2};
use craft::theme::Srgba;
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::runtime::env::Env;
use craft::ui::testing::Fixture;
use rivercrossing_cards::{JokersMode, Shoe};
use rivercrossing_ride::{
    EntryMode, PlateModel, RideConfig, RideConfigParams, RideEngine, SystemClock,
};
use rivercrossing_roster::roster::Roster;
use rstest::rstest;

use super::{
    MIXED_RIDES_ONLY, NEEDS_RIDE, NO_AUDIT_ROWS, NOT_AVAILABLE_NOW, header, rail_entry, rail_hint,
};
use crate::ui::action::enable::{GateState, RidePhase};
use crate::ui::action::id::{
    ActionId, MI_AUDIT_TRAIL, MI_CHECK_RIDER_ISSUES, MI_OPEN_LIBRARY, MI_RIDER_EDITOR,
    MI_STANDINGS, MI_START_RIDE, MI_TEAM_EDITOR, WX_ID_PREFERENCES,
};
use crate::ui::state::{AppState, LoadedRide};

/// Return the gate a running mixed ride gives, with an audit row when `has_audit_row` is set.
fn mixed_running_gate(has_audit_row: bool) -> GateState {
    GateState {
        phase: RidePhase::Running,
        teams_enabled: true,
        has_crossing: true,
        has_audit_row,
        ..GateState::default()
    }
}

/// Return the gate a running solo ride with an audit row gives.
fn solo_running_gate() -> GateState {
    GateState {
        phase: RidePhase::Running,
        teams_enabled: false,
        has_crossing: true,
        has_audit_row: true,
        ..GateState::default()
    }
}

// ------------------------------------------------------------------ rail hints

#[rstest]
#[case::library(MI_OPEN_LIBRARY, None)]
#[case::riders(MI_RIDER_EDITOR, Some(NEEDS_RIDE))]
#[case::teams(MI_TEAM_EDITOR, Some(NEEDS_RIDE))]
#[case::issues(MI_CHECK_RIDER_ISSUES, Some(NEEDS_RIDE))]
#[case::standings(MI_STANDINGS, None)]
#[case::audit(MI_AUDIT_TRAIL, Some(NEEDS_RIDE))]
#[case::settings(WX_ID_PREFERENCES, None)]
fn test_rail_hint_with_no_ride_names_the_missing_ride(
    #[case] action: ActionId,
    #[case] expected: Option<&'static str>,
) {
    // Arrange
    let gate = GateState::default();

    // Act
    let hint = rail_hint(action, gate);

    // Assert
    assert_eq!(hint, expected);
}

#[rstest]
#[case::library(MI_OPEN_LIBRARY, None)]
#[case::riders(MI_RIDER_EDITOR, None)]
#[case::teams(MI_TEAM_EDITOR, None)]
#[case::issues(MI_CHECK_RIDER_ISSUES, None)]
#[case::standings(MI_STANDINGS, None)]
#[case::audit(MI_AUDIT_TRAIL, Some(NO_AUDIT_ROWS))]
#[case::settings(WX_ID_PREFERENCES, None)]
fn test_rail_hint_on_a_running_mixed_ride_names_the_missing_audit_row(
    #[case] action: ActionId,
    #[case] expected: Option<&'static str>,
) {
    // Arrange: a mixed ride with crossings but no audit row.
    let gate = mixed_running_gate(false);

    // Act
    let hint = rail_hint(action, gate);

    // Assert
    assert_eq!(hint, expected);
}

#[rstest]
#[case::library(MI_OPEN_LIBRARY)]
#[case::riders(MI_RIDER_EDITOR)]
#[case::teams(MI_TEAM_EDITOR)]
#[case::issues(MI_CHECK_RIDER_ISSUES)]
#[case::standings(MI_STANDINGS)]
#[case::audit(MI_AUDIT_TRAIL)]
#[case::settings(WX_ID_PREFERENCES)]
fn test_rail_hint_on_a_running_mixed_ride_with_an_audit_row_gives_no_hint(
    #[case] action: ActionId,
) {
    // Arrange: every rail gate passes.
    let gate = mixed_running_gate(true);

    // Act
    let hint = rail_hint(action, gate);

    // Assert
    assert_eq!(hint, None);
}

#[rstest]
#[case::library(MI_OPEN_LIBRARY, None)]
#[case::riders(MI_RIDER_EDITOR, None)]
#[case::teams(MI_TEAM_EDITOR, Some(MIXED_RIDES_ONLY))]
#[case::issues(MI_CHECK_RIDER_ISSUES, None)]
#[case::standings(MI_STANDINGS, None)]
#[case::audit(MI_AUDIT_TRAIL, None)]
#[case::settings(WX_ID_PREFERENCES, None)]
fn test_rail_hint_on_a_running_solo_ride_names_mixed_rides_only(
    #[case] action: ActionId,
    #[case] expected: Option<&'static str>,
) {
    // Arrange
    let gate = solo_running_gate();

    // Act
    let hint = rail_hint(action, gate);

    // Assert
    assert_eq!(hint, expected);
}

#[test]
fn test_rail_hint_on_a_muted_action_outside_the_rail_says_not_available() {
    // Arrange: a running mixed ride, where Start Ride has already passed its gate.
    let gate = mixed_running_gate(true);

    // Act
    let hint = rail_hint(MI_START_RIDE, gate);

    // Assert
    assert_eq!(hint, Some(NOT_AVAILABLE_NOW));
}

// ------------------------------------------------------------- the hover state

/// Return the fill colour a rail entry paints under the pointer.
fn hover_fill(ui: &Ui) -> Color32 {
    Color32::from(Srgba::from(Env::of(ui).tokens.color.state.hover))
}

/// Return the rects the frame filled with *fill*.
fn filled_cells(shapes: &[Shape], fill: Color32) -> Vec<Rect> {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Rect(rect) if rect.fill == fill => Some(rect.rect),
            _ => None,
        })
        .collect()
}

/// Return a fixture that shows two rail entries, the second one live when `second_enabled` is set.
fn two_entry_fixture(second_enabled: bool) -> (Fixture, Color32) {
    let fill = Rc::new(Cell::new(Color32::TRANSPARENT));
    let painted = Rc::clone(&fill);
    let mut fixture = Fixture::new(move |ui| {
        painted.set(hover_fill(ui));
        ui.push_id("library", |ui| {
            let _ = rail_entry(ui, "Library", "folder_open", true, None);
        });
        ui.push_id("riders", |ui| {
            let _ = rail_entry(ui, "Riders", "grid", second_enabled, None);
        });
    });
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(1);
    (fixture, fill.get())
}

#[test]
fn test_rail_entry_hover_fills_only_the_entry_under_the_pointer() {
    // Arrange: two enabled entries.
    let (mut fixture, fill) = two_entry_fixture(true);
    let library = fixture.by_label("Library").expect("the first rail entry");
    let riders = fixture.by_label("Riders").expect("the second rail entry");

    // Act: the pointer rests on the first entry.
    let shapes = fixture.frame_shapes_with(vec![Event::PointerMoved(library.center())]);

    // Assert
    let cells = filled_cells(&shapes, fill);
    assert!(!cells.is_empty(), "the entry under the pointer fills");
    assert!(
        cells
            .iter()
            .all(|cell| cell.contains(library.center()) && !cell.contains(riders.center())),
        "the hover belongs to the entry under the pointer alone"
    );
}

#[test]
fn test_rail_entry_hover_fills_the_second_entry_at_its_own_place() {
    // Arrange: two enabled entries.
    let (mut fixture, fill) = two_entry_fixture(true);
    let library = fixture.by_label("Library").expect("the first rail entry");
    let riders = fixture.by_label("Riders").expect("the second rail entry");

    // Act: the pointer rests on the second entry.
    let shapes = fixture.frame_shapes_with(vec![Event::PointerMoved(riders.center())]);

    // Assert
    let cells = filled_cells(&shapes, fill);
    assert!(!cells.is_empty(), "the entry under the pointer fills");
    assert!(
        cells
            .iter()
            .all(|cell| cell.contains(riders.center()) && !cell.contains(library.center())),
        "the hover follows the pointer to the second entry"
    );
}

#[test]
fn test_rail_entry_hover_leaves_a_muted_entry_unfilled() {
    // Arrange: the second entry is muted.
    let (mut fixture, fill) = two_entry_fixture(false);
    let riders = fixture.by_label("Riders").expect("the second rail entry");

    // Act: the pointer rests on the muted entry.
    let shapes = fixture.frame_shapes_with(vec![Event::PointerMoved(riders.center())]);

    // Assert
    assert_eq!(filled_cells(&shapes, fill), Vec::<Rect>::new());
}

// ------------------------------------------------------------------ the ride header

/// Return a loaded ride carrying the mockup's identity.
fn fixture_ride() -> LoadedRide {
    let config = RideConfig::new(RideConfigParams {
        name: "GORBA EPIC 2026".to_owned(),
        event_date: time::Date::from_calendar_date(2026, time::Month::June, 13)
            .expect("the mockup date"),
        venue: "GORBA Park".to_owned(),
        lap_km: 7.5,
        organizer: "GORBA".to_owned(),
        scorer: "M. Buckaway".to_owned(),
        min_lap_s: 1200,
        planned_duration_s: 14_400,
        ..RideConfigParams::default()
    })
    .expect("the fixture config");
    let roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(7))
        .expect("the fixture roster");
    let shoe = Shoe::new(8, 1, 7, JokersMode::Total).expect("the fixture shoe");
    let engine = RideEngine::new(config.clone(), shoe, SystemClock, roster.clone());
    LoadedRide {
        ride_id: 1,
        config,
        roster,
        engine,
    }
}

/// Return a fixture showing the console's ride header in a wide room.
fn header_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        header(ui, &mut shown.borrow_mut(), 1000.0);
    });
    fixture.room(Vec2::new(1100.0, 200.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(2);
    fixture
}

#[test]
fn test_header_with_no_ride_writes_an_em_dash_under_every_caption() {
    // Arrange: the launch state, with no ride loaded.
    let state = Rc::new(RefCell::new(AppState::blank()));

    // Act
    let fixture = header_fixture(&state);
    let shown: Vec<&str> = fixture
        .texts()
        .iter()
        .map(|run| run.text.as_str())
        .collect();

    // Assert: one badge, the logo tile, then every caption over its value, and no empty string.
    let expected = [
        "NO RIDE",
        "LOGO",
        "Ride",
        "—",
        "Date",
        "—",
        "Venue",
        "—",
        "Organizer",
        "—",
        "Scorer",
        "—",
        "Lap km",
        "—",
        "Current lap",
        "—",
    ];
    assert_eq!(shown, expected);
}

#[test]
fn test_header_with_a_ride_shows_each_value_under_its_frozen_id() {
    // Arrange
    let state = Rc::new(RefCell::new(AppState::blank()));
    state.borrow_mut().ride = Some(fixture_ride());

    // Act
    let fixture = header_fixture(&state);
    let name_of = |id: &str| {
        fixture
            .node_of(Id::unique(id))
            .and_then(|target| fixture.tree().name_of(target.node))
            .map(str::to_owned)
    };

    // Assert: the frozen ids survive, and each shows the presenter's value.
    assert_eq!(
        name_of("ride_name_value"),
        Some("GORBA EPIC 2026".to_owned())
    );
    assert_eq!(name_of("ride_date_value"), Some("2026-06-13".to_owned()));
    assert_eq!(name_of("ride_venue_value"), Some("GORBA Park".to_owned()));
    assert_eq!(name_of("ride_organizer_value"), Some("GORBA".to_owned()));
    assert_eq!(name_of("ride_scorer_value"), Some("M. Buckaway".to_owned()));
    assert_eq!(name_of("ride_lap_km_value"), Some("7.5".to_owned()));
    assert_eq!(name_of("current_lap_lbl"), Some("0".to_owned()));
}

// -------------------------------------------------------- export outcome

/// Return a fixture showing the status bar over *state*.
fn status_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        super::status_bar(ui, &mut shown.borrow_mut());
    });
    fixture.room(Vec2::new(900.0, 120.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(3);
    fixture
}

#[test]
fn test_status_bar_given_a_finished_export_names_the_file_and_offers_open() {
    // Arrange
    let mut state = AppState::blank();
    state.exported_file = Some(std::path::PathBuf::from("/tmp/results.html"));
    let state = Rc::new(RefCell::new(state));

    // Act
    let fixture = status_fixture(&state);
    let texts: Vec<&str> = fixture
        .texts()
        .iter()
        .map(|run| run.text.as_str())
        .collect();

    // Assert: the notice names the file, not the whole path, and offers Open.
    assert!(
        texts.contains(&"Exported results.html"),
        "the success notice names the file: {texts:?}"
    );
    assert!(texts.contains(&"Open"), "the notice is actionable");
}

#[test]
fn test_status_bar_given_the_open_action_queues_the_file() {
    // Arrange
    let path = std::path::PathBuf::from("/tmp/results.html");
    let mut state = AppState::blank();
    state.exported_file = Some(path.clone());
    let state = Rc::new(RefCell::new(state));
    let mut fixture = status_fixture(&state);
    let open = fixture.by_label("Open").expect("the Open action");

    // Act
    fixture.click(&open);

    // Assert
    let queued = state.borrow().open_export.clone();
    assert_eq!(queued, Some(path));
}
