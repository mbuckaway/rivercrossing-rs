// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the standings window's notice order and its Solo pane.

use std::cell::RefCell;
use std::rc::Rc;

use craft::egui::{Rect, Vec2};
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::testing::Fixture;
use rivercrossing_cards::{JokersMode, Shoe};
use rivercrossing_ride::{
    EntryMode, PlateModel, RideConfig, RideConfigParams, RideEngine, SnapshotKind, SystemClock,
};
use rivercrossing_roster::roster::Roster;

use super::body;
use crate::ui::presenters::results::{StandingsModel, StandingsRowModel};
use crate::ui::state::exports::ResultsTab;
use crate::ui::state::{AppState, LoadedRide};

/// The ride name the fixture uses.
const RIDE_NAME: &str = "GORBA EPIC 2026";

/// The seed every fixture ride stores.
const SEED: u64 = 7;

/// Return one standings row.
fn row(place: u32, name: &str, kind: SnapshotKind) -> StandingsRowModel {
    StandingsRowModel {
        place: Some(place),
        plate: "123".to_owned(),
        name: name.to_owned(),
        kind,
        sex: None,
        laps: 8,
        hand: "Full House — Aces over Fours".to_owned(),
        draw: "2♥".to_owned(),
        total_time: 5_640.0,
        best_lap: 1_318.0,
    }
}

/// Return a loaded mixed ride from the in-memory engine.
fn ride() -> LoadedRide {
    let config = RideConfig::new(RideConfigParams {
        name: RIDE_NAME.to_owned(),
        event_date: time::Date::from_calendar_date(2026, time::Month::June, 13)
            .expect("the mockup date"),
        venue: "GORBA Park".to_owned(),
        lap_km: 7.5,
        organizer: "GORBA".to_owned(),
        scorer: "M. Buckaway".to_owned(),
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        min_lap_s: 1200,
        planned_duration_s: 14_400,
        ..RideConfigParams::default()
    })
    .expect("the fixture config");
    let roster =
        Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED)).expect("the roster");
    let shoe = Shoe::new(8, 1, 7, JokersMode::Total).expect("the fixture shoe");
    let engine = RideEngine::new(config.clone(), shoe, SystemClock, roster.clone());
    LoadedRide {
        ride_id: 1,
        config,
        roster,
        engine,
    }
}

/// Return a state with the fixture ride and *model*, showing *tab*.
fn results_state(model: StandingsModel, tab: ResultsTab) -> Rc<RefCell<AppState>> {
    let state = Rc::new(RefCell::new(AppState::blank()));
    {
        let mut app = state.borrow_mut();
        app.ride = Some(ride());
        app.results.model = Some(model);
        app.results.tab = tab;
    }
    state
}

/// Return a fixture showing the standings body over *state*.
fn results_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        body(ui, &mut shown.borrow_mut());
    });
    fixture.room(Vec2::new(1000.0, 720.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(3);
    fixture
}

/// Return the rect of the first painted run whose text is exactly *text*.
fn text_rect(fixture: &Fixture, text: &str) -> Rect {
    fixture
        .texts()
        .iter()
        .find(|run| run.text == text)
        .map(|run| run.rect)
        .expect("the run shows")
}

#[test]
fn test_results_given_a_stale_export_puts_the_notice_above_the_tabs() {
    // Arrange
    let model = StandingsModel {
        teams: vec![row(1, "River Rats", SnapshotKind::Team)],
        solo: Vec::new(),
        stale: true,
        self_test_unverified: false,
    };
    let state = results_state(model, ResultsTab::Teams);

    // Act
    let fixture = results_fixture(&state);
    let notice = text_rect(
        &fixture,
        "A correction landed after the last export; export again.",
    );
    let teams = text_rect(&fixture, "Teams");

    // Assert: the design puts the notice above the notebook (`index.html:496`).
    assert!(
        notice.max.y <= teams.min.y,
        "the notice leads the tab strip: {notice:?} {teams:?}"
    );
}

#[test]
fn test_results_given_the_solo_tab_renders_the_solo_table() {
    // Arrange
    let model = StandingsModel {
        teams: vec![row(1, "River Rats", SnapshotKind::Team)],
        solo: vec![row(1, "Solo Star", SnapshotKind::Solo)],
        stale: false,
        self_test_unverified: false,
    };
    let state = results_state(model, ResultsTab::Solo);

    // Act
    let fixture = results_fixture(&state);
    let entry = text_rect(&fixture, "Solo Star");

    // Assert: the Solo pane paints its own row, not an empty label.
    assert!(entry.width() > 1.0, "the solo row shows: {entry:?}");
}

#[test]
fn test_results_given_the_solo_tab_uses_its_own_columns() {
    // Arrange
    let model = StandingsModel {
        teams: vec![row(1, "River Rats", SnapshotKind::Team)],
        solo: vec![row(1, "Solo Star", SnapshotKind::Solo)],
        stale: false,
        self_test_unverified: false,
    };
    let state = results_state(model, ResultsTab::Solo);

    // Act
    let fixture = results_fixture(&state);

    // Assert: the Solo pane's own column set, its Type column dropped and its Draw column shown.
    let candidates = [
        "Place", "Plate", "Entry", "Type", "Laps", "Best 5", "Hand", "Draw", "Total",
    ];
    let mut headers: Vec<(f32, &str)> = fixture
        .texts()
        .iter()
        .filter(|run| candidates.contains(&run.text.as_str()))
        .map(|run| (run.rect.min.x, run.text.as_str()))
        .collect();
    headers.sort_by(|left, right| left.0.total_cmp(&right.0));
    let shown: Vec<&str> = headers.into_iter().map(|(_, text)| text).collect();
    assert_eq!(
        shown,
        ["Place", "Plate", "Entry", "Laps", "Hand", "Draw", "Total"]
    );
}
