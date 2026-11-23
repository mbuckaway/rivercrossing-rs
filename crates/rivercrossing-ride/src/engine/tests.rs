// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the ride engine: construction, lifecycle, timing,
//! crossing recording, and the config update.

#![expect(
    clippy::float_cmp,
    reason = "fixture instants are whole seconds, so every derived value is exact"
)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use pretty_assertions::assert_eq;
use rivercrossing_cards::{Card, JokersMode, Shoe, high_card_draw};
use rivercrossing_hands::best_hand;
use serde_json::json;
use time::{Date, Duration, Month, PrimitiveDateTime, Time};

use super::{
    Crossing, CrossingResult, HeldCrossing, PendingMiss, RideEngine, SnapshotEntry, SnapshotKind,
};
use crate::clock::{Clock, Timestamp};
use crate::config::{EntryMode, PlateModel, RideConfig, RideConfigParams};
use crate::error::RideError;
use crate::event::{Event, EventAction};
use crate::roster_view::{EntryId, EntrySnapshot, RiderSnapshot, RosterView, StartViolation};
use crate::status::RideStatus;

/// The seed every fixture shoe uses, matching the oracle's fixture seed.
const SEED: u64 = 20_260_920;

// ------------------------------------------------------------- fixtures

fn fixture_date() -> Date {
    Date::from_calendar_date(2026, Month::September, 20).expect("the fixture date is valid")
}

fn at(hour: u8, minute: u8, second: u8) -> Timestamp {
    let time = Time::from_hms(hour, minute, second).expect("the fixture time is valid");
    PrimitiveDateTime::new(fixture_date(), time).assume_utc()
}

fn params() -> RideConfigParams {
    RideConfigParams {
        name: "GORBA EPIC 2026".to_owned(),
        event_date: fixture_date(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        planned_start: at(10, 0, 0),
        planned_duration_s: 21_600,
        min_lap_s: 1_080,
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        ..RideConfigParams::default()
    }
}

fn fixture_config() -> RideConfig {
    RideConfig::new(params()).expect("the fixture config is valid")
}

fn config_with(changes: RideConfigParams) -> RideConfig {
    RideConfig::new(changes).expect("the changed config is valid")
}

fn solo_entry(plate: &str) -> EntrySnapshot {
    EntrySnapshot {
        id: EntryId(format!("key-{plate}")),
        plate: plate.to_owned(),
        display_name: format!("Rider {plate}"),
        is_team: false,
        is_dnf: false,
        riders: vec![RiderSnapshot {
            plate: Some(plate.to_owned()),
            full_name: format!("Rider {plate}"),
            sex: None,
        }],
    }
}

fn two_entries() -> Vec<EntrySnapshot> {
    vec![solo_entry("12"), solo_entry("34")]
}

fn pooled_team() -> EntrySnapshot {
    EntrySnapshot {
        id: EntryId("key-team".to_owned()),
        plate: "9".to_owned(),
        display_name: "Dirt Dynamos".to_owned(),
        is_team: true,
        is_dnf: false,
        riders: vec![
            RiderSnapshot {
                plate: Some("45".to_owned()),
                full_name: "Sarah".to_owned(),
                sex: None,
            },
            RiderSnapshot {
                plate: Some("9".to_owned()),
                full_name: "Priya".to_owned(),
                sex: None,
            },
        ],
    }
}

fn relay_team() -> EntrySnapshot {
    EntrySnapshot {
        id: EntryId("key-relay".to_owned()),
        plate: "77".to_owned(),
        display_name: "Road Crew".to_owned(),
        is_team: true,
        is_dnf: false,
        riders: vec![RiderSnapshot {
            plate: None,
            full_name: "Ana".to_owned(),
            sex: None,
        }],
    }
}

/// Return a two-rider pooled team with its own derived plate.
#[expect(
    clippy::too_many_arguments,
    reason = "the fixture names the entry's key, plate, display name and two riders"
)]
fn pooled_team_entry(
    id: &str,
    plate: &str,
    name: &str,
    riders: [(&str, &str); 2],
) -> EntrySnapshot {
    EntrySnapshot {
        id: EntryId(id.to_owned()),
        plate: plate.to_owned(),
        display_name: name.to_owned(),
        is_team: true,
        is_dnf: false,
        riders: riders
            .into_iter()
            .map(|(rider_plate, full_name)| RiderSnapshot {
                plate: Some(rider_plate.to_owned()),
                full_name: full_name.to_owned(),
                sex: None,
            })
            .collect(),
    }
}

fn team_alpha() -> EntrySnapshot {
    pooled_team_entry("key-team-a", "1", "Team A", [("1", "Ada"), ("2", "Bea")])
}

fn team_beta() -> EntrySnapshot {
    pooled_team_entry("key-team-b", "3", "Team B", [("3", "Cleo"), ("4", "Dana")])
}

fn two_team_pooled() -> Vec<EntrySnapshot> {
    vec![team_alpha(), team_beta()]
}

fn solo_and_team() -> Vec<EntrySnapshot> {
    vec![solo_entry("5"), team_beta()]
}

// --------------------------------------------------------- the doubles

/// One captured `set_entry_dnf` call.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DnfWrite {
    id: EntryId,
    plate: String,
    rider: bool,
    reason: String,
}

#[derive(Debug, Default)]
struct RosterState {
    entries: Vec<EntrySnapshot>,
    statuses: Vec<RideStatus>,
    data: Vec<EntryId>,
    dnf: Vec<DnfWrite>,
    moves: Vec<(String, EntryId, String)>,
    extracts: Vec<(String, String)>,
    fail_moves: bool,
    violations: Vec<StartViolation>,
}

/// The roster seam's test implementation: in-memory entries plus a
/// capture of every write the engine makes.
#[derive(Debug, Clone, Default)]
struct FakeRoster {
    state: Rc<RefCell<RosterState>>,
}

impl FakeRoster {
    fn new(entries: Vec<EntrySnapshot>) -> Self {
        Self {
            state: Rc::new(RefCell::new(RosterState {
                entries,
                ..RosterState::default()
            })),
        }
    }

    fn with_violations(entries: Vec<EntrySnapshot>, violations: Vec<StartViolation>) -> Self {
        Self {
            state: Rc::new(RefCell::new(RosterState {
                entries,
                violations,
                ..RosterState::default()
            })),
        }
    }

    fn refusing_moves(entries: Vec<EntrySnapshot>) -> Self {
        Self {
            state: Rc::new(RefCell::new(RosterState {
                entries,
                fail_moves: true,
                ..RosterState::default()
            })),
        }
    }

    fn statuses(&self) -> Vec<RideStatus> {
        self.state.borrow().statuses.clone()
    }

    fn has_data(&self) -> Vec<EntryId> {
        self.state.borrow().data.clone()
    }

    fn dnf_writes(&self) -> Vec<DnfWrite> {
        self.state.borrow().dnf.clone()
    }

    fn moves(&self) -> Vec<(String, EntryId, String)> {
        self.state.borrow().moves.clone()
    }

    fn extracts(&self) -> Vec<(String, String)> {
        self.state.borrow().extracts.clone()
    }

    fn plates(&self) -> Vec<String> {
        self.state
            .borrow()
            .entries
            .iter()
            .map(|entry| entry.plate.clone())
            .collect()
    }

    fn add_entry(&self, entry: EntrySnapshot) {
        self.state.borrow_mut().entries.push(entry);
    }
}

/// Re-derive a pooled team's plate from its lowest-numbered rider (S1).
fn rederive_team_plate(entry: &mut EntrySnapshot) {
    let lowest = entry
        .riders
        .iter()
        .filter_map(|rider| rider.plate.as_deref())
        .filter_map(|plate| {
            plate
                .parse::<u32>()
                .ok()
                .map(|value| (value, plate.to_owned()))
        })
        .min();
    if let Some((_value, plate)) = lowest {
        entry.plate = plate;
    }
}

/// Re-derive every pooled team's plate after a membership change.
fn rederive_plates(entries: &mut [EntrySnapshot]) {
    for entry in entries {
        if entry.is_team {
            rederive_team_plate(entry);
        }
    }
}

/// Remove `rider_plate`'s rider from the roster, dissolving an entry that
/// loses its last rider, exactly as the roster crate's move does.
fn take_rider(entries: &mut Vec<EntrySnapshot>, rider_plate: &str) -> Option<RiderSnapshot> {
    let position = entries.iter().position(|entry| {
        entry
            .riders
            .iter()
            .any(|rider| rider.plate.as_deref() == Some(rider_plate))
    })?;
    let entry = entries.get_mut(position)?;
    let rider_position = entry
        .riders
        .iter()
        .position(|rider| rider.plate.as_deref() == Some(rider_plate))?;
    let rider = entry.riders.remove(rider_position);
    if entry.riders.is_empty() {
        entries.remove(position);
    }
    Some(rider)
}

impl RosterView for FakeRoster {
    fn entries(&self) -> Vec<EntrySnapshot> {
        self.state.borrow().entries.clone()
    }

    fn resolve_plate(&self, plate: &str) -> Option<EntrySnapshot> {
        self.state
            .borrow()
            .entries
            .iter()
            .find(|entry| {
                entry.plate == plate
                    || entry
                        .riders
                        .iter()
                        .any(|rider| rider.plate.as_deref() == Some(plate))
            })
            .cloned()
    }

    fn entry(&self, id: &EntryId) -> Option<EntrySnapshot> {
        self.state
            .borrow()
            .entries
            .iter()
            .find(|entry| &entry.id == id)
            .cloned()
    }

    fn start_violations(&self) -> Vec<StartViolation> {
        self.state.borrow().violations.clone()
    }

    fn set_ride_status(&mut self, status: RideStatus) {
        self.state.borrow_mut().statuses.push(status);
    }

    fn mark_has_data(&mut self, id: &EntryId) {
        self.state.borrow_mut().data.push(id.clone());
    }

    fn set_entry_dnf(&mut self, id: &EntryId, plate: &str, rider: bool, reason: &str) {
        let mut state = self.state.borrow_mut();
        state.dnf.push(DnfWrite {
            id: id.clone(),
            plate: plate.to_owned(),
            rider,
            reason: reason.to_owned(),
        });
        if !rider && let Some(entry) = state.entries.iter_mut().find(|entry| &entry.id == id) {
            entry.is_dnf = true;
        }
    }

    fn move_rider(
        &mut self,
        rider_plate: &str,
        to_entry: &EntryId,
        reason: &str,
    ) -> Result<(), RideError> {
        let mut state = self.state.borrow_mut();
        state
            .moves
            .push((rider_plate.to_owned(), to_entry.clone(), reason.to_owned()));
        if state.fail_moves {
            return Err(RideError::Roster {
                message: "the roster refused the move".to_owned(),
            });
        }
        let Some(rider) = take_rider(&mut state.entries, rider_plate) else {
            return Err(RideError::UnknownPlate {
                plate: rider_plate.to_owned(),
            });
        };
        if let Some(destination) = state.entries.iter_mut().find(|entry| &entry.id == to_entry) {
            destination.riders.push(rider);
        }
        rederive_plates(&mut state.entries);
        Ok(())
    }

    fn extract_rider_to_solo(
        &mut self,
        rider_plate: &str,
        reason: &str,
    ) -> Result<EntryId, RideError> {
        let mut state = self.state.borrow_mut();
        state
            .extracts
            .push((rider_plate.to_owned(), reason.to_owned()));
        if state.fail_moves {
            return Err(RideError::Roster {
                message: "the roster refused the extraction".to_owned(),
            });
        }
        let Some(rider) = take_rider(&mut state.entries, rider_plate) else {
            return Err(RideError::UnknownPlate {
                plate: rider_plate.to_owned(),
            });
        };
        let id = EntryId(format!("key-solo-{rider_plate}"));
        rederive_plates(&mut state.entries);
        state.entries.push(EntrySnapshot {
            id: id.clone(),
            plate: rider_plate.to_owned(),
            display_name: rider.full_name.clone(),
            is_team: false,
            is_dnf: false,
            riders: vec![rider],
        });
        Ok(id)
    }
}

/// The clock seam's test implementation: a shared epoch second a test
/// can move forward.
#[derive(Debug, Clone)]
struct FixedClock {
    epoch_seconds: Rc<Cell<i64>>,
}

impl FixedClock {
    fn new(now: Timestamp) -> Self {
        Self {
            epoch_seconds: Rc::new(Cell::new(now.unix_timestamp())),
        }
    }

    fn advance(&self, seconds: i64) {
        self.epoch_seconds.set(self.epoch_seconds.get() + seconds);
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::UNIX_EPOCH + Duration::seconds(self.epoch_seconds.get())
    }
}

// ---------------------------------------------------------- the engine

struct Fixture {
    engine: RideEngine<FakeRoster, FixedClock>,
    clock: FixedClock,
    roster: FakeRoster,
}

fn fixture(config: RideConfig, roster: FakeRoster) -> Fixture {
    fixture_with_seed(config, roster, SEED)
}

fn fixture_with_seed(config: RideConfig, roster: FakeRoster, seed: u64) -> Fixture {
    let clock = FixedClock::new(config.planned_start);
    let decks = u8::try_from(config.deck_count).expect("the fixture deck count fits u8");
    let jokers = u8::try_from(config.jokers_per_deck).expect("the fixture joker count fits u8");
    let shoe = Shoe::new(decks, jokers, seed, config.jokers_mode).expect("the shoe config valid");
    let engine = RideEngine::new(config, shoe, clock.clone(), roster.clone());
    Fixture {
        engine,
        clock,
        roster,
    }
}

fn default_fixture() -> Fixture {
    fixture(fixture_config(), FakeRoster::new(two_entries()))
}

fn running_fixture() -> Fixture {
    let mut fixture = default_fixture();
    fixture.engine.start(None).expect("the roster is ready");
    fixture
}

fn stopped_fixture() -> Fixture {
    let mut fixture = running_fixture();
    fixture.engine.stop().expect("the ride is running");
    fixture
}

fn finished_fixture() -> Fixture {
    let mut fixture = running_fixture();
    fixture.engine.finish(&[]).expect("the ride is running");
    fixture
}

fn reopened_fixture() -> Fixture {
    let mut fixture = finished_fixture();
    fixture.engine.reopen().expect("the ride is finished");
    fixture
}

fn key(roster: &FakeRoster, plate: &str) -> EntryId {
    roster
        .resolve_plate(plate)
        .expect("the fixture roster holds the plate")
        .id
}

fn refused(plate: &str, reason: &str) -> CrossingResult {
    CrossingResult {
        accepted: false,
        plate: plate.to_owned(),
        entry_plate: None,
        entry_name: None,
        lap: 0,
        lap_time: 0.0,
        reason: Some(reason.to_owned()),
        card: None,
        flagged: false,
    }
}

fn reference_card(config: &RideConfig, deals: usize) -> Card {
    let decks = u8::try_from(config.deck_count).expect("the deck count fits u8");
    let jokers = u8::try_from(config.jokers_per_deck).expect("the joker count fits u8");
    let mut shoe =
        Shoe::new(decks, jokers, SEED, config.jokers_mode).expect("the shoe config valid");
    let mut card = Card::Joker;
    for _ in 0..=deals {
        card = shoe.deal().expect("the reference shoe deals").0;
    }
    card
}

#[expect(
    clippy::too_many_arguments,
    reason = "the test helper mirrors the engine, plate, count, start and step of a batch"
)]
fn record_many(
    engine: &mut RideEngine<FakeRoster, FixedClock>,
    plate: &str,
    count: u32,
    start: Timestamp,
    step_seconds: i64,
) {
    for index in 0..count {
        let instant = start + Duration::seconds(i64::from(index) * step_seconds);
        let _ = engine.record_crossing(plate, Some(instant));
    }
}

fn illegal(message: &str) -> RideError {
    RideError::IllegalState {
        message: message.to_owned(),
    }
}

// ----------------------------------------------------------- new/state

#[test]
fn test_new_builds_a_draft_engine_with_no_events() {
    // Arrange / Act
    let fixture = default_fixture();

    // Assert
    assert_eq!(fixture.engine.state(), RideStatus::Draft);
    assert_eq!(fixture.engine.events(), Vec::new());
    assert_eq!(fixture.engine.stopped(), false);
    assert_eq!(fixture.engine.actual_start(), None);
}

#[test]
fn test_new_holds_the_given_config() {
    // Arrange
    let config = fixture_config();

    // Act
    let fixture = fixture(config.clone(), FakeRoster::new(two_entries()));

    // Assert
    assert_eq!(fixture.engine.config(), &config);
}

// -------------------------------------------------------------- start

#[test]
fn test_start_from_draft_moves_to_running_and_appends_the_start_event() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let event = fixture.engine.start(None).expect("the roster is ready");

    // Assert
    assert_eq!(fixture.engine.state(), RideStatus::Running);
    assert_eq!(
        event,
        Event {
            action: EventAction::Start,
            payload: json!({
                "actual_start": "2026-09-20T10:00:00",
                "reason": "0:00:00",
            }),
        }
    );
    assert_eq!(fixture.engine.events().to_vec(), vec![event]);
}

#[test]
fn test_start_from_draft_sets_the_roster_status_to_running() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    fixture.engine.start(None).expect("the roster is ready");

    // Assert
    assert_eq!(fixture.roster.statuses(), vec![RideStatus::Running]);
}

#[test]
fn test_start_with_an_explicit_instant_sets_actual_start() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    fixture
        .engine
        .start(Some(at(9, 45, 0)))
        .expect("the roster is ready");

    // Assert
    assert_eq!(fixture.engine.actual_start(), Some(at(9, 45, 0)));
    assert_eq!(
        fixture.engine.events().last(),
        Some(&Event {
            action: EventAction::Start,
            payload: json!({
                "actual_start": "2026-09-20T09:45:00",
                "reason": "0:00:00",
            }),
        })
    );
}

#[test]
fn test_start_without_an_instant_uses_the_clock() {
    // Arrange
    let mut fixture = default_fixture();
    fixture.clock.advance(30);

    // Act
    fixture.engine.start(None).expect("the roster is ready");

    // Assert
    assert_eq!(fixture.engine.actual_start(), Some(at(10, 0, 30)));
}

#[test]
fn test_start_with_an_empty_roster_is_blocked_with_its_reason() {
    // Arrange
    let mut fixture = fixture(fixture_config(), FakeRoster::new(Vec::new()));

    // Act
    let result = fixture.engine.start(None);

    // Assert
    assert_eq!(
        result,
        Err(RideError::StartBlocked {
            message: "roster has no riders".to_owned(),
            reasons: vec!["roster has no riders".to_owned()],
        })
    );
    assert_eq!(fixture.engine.state(), RideStatus::Draft);
    assert_eq!(fixture.engine.events(), Vec::new());
}

#[test]
fn test_start_with_a_missing_setup_field_is_blocked_with_its_reason() {
    // Arrange
    let config = config_with(RideConfigParams {
        venue: String::new(),
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));

    // Act
    let result = fixture.engine.start(None);

    // Assert
    assert_eq!(
        result,
        Err(RideError::StartBlocked {
            message: "ride setup is incomplete: venue is required".to_owned(),
            reasons: vec!["venue is required".to_owned()],
        })
    );
    assert_eq!(fixture.engine.state(), RideStatus::Draft);
}

#[test]
fn test_start_with_several_missing_setup_fields_joins_every_reason() {
    // Arrange
    let config = config_with(RideConfigParams {
        name: String::new(),
        venue: " ".to_owned(),
        lap_km: 0.0,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));

    // Act
    let result = fixture.engine.start(None);

    // Assert
    assert_eq!(
        result,
        Err(RideError::StartBlocked {
            message: "ride setup is incomplete: name is required; venue is required; lap length \
                      must be positive"
                .to_owned(),
            reasons: vec![
                "name is required".to_owned(),
                "venue is required".to_owned(),
                "lap length must be positive".to_owned(),
            ],
        })
    );
}

#[test]
fn test_start_with_roster_violations_prefixes_each_plate() {
    // Arrange
    let violations = vec![StartViolation {
        plate: "7".to_owned(),
        reason: "team size must be at least 2, got 1".to_owned(),
    }];
    let mut fixture = fixture(
        fixture_config(),
        FakeRoster::with_violations(vec![solo_entry("7")], violations),
    );

    // Act
    let result = fixture.engine.start(None);

    // Assert
    assert_eq!(
        result,
        Err(RideError::StartBlocked {
            message: "roster is not ready to start: 7: team size must be at least 2, got 1"
                .to_owned(),
            reasons: vec!["7: team size must be at least 2, got 1".to_owned()],
        })
    );
}

#[test]
fn test_start_checks_the_empty_roster_before_the_setup() {
    // Arrange
    let config = config_with(RideConfigParams {
        venue: String::new(),
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(Vec::new()));

    // Act
    let result = fixture.engine.start(None);

    // Assert
    assert_eq!(
        result,
        Err(RideError::StartBlocked {
            message: "roster has no riders".to_owned(),
            reasons: vec!["roster has no riders".to_owned()],
        })
    );
}

#[test]
fn test_start_checks_the_setup_before_the_roster_violations() {
    // Arrange
    let violations = vec![StartViolation {
        plate: "12".to_owned(),
        reason: "team size must be at least 2, got 1".to_owned(),
    }];
    let config = config_with(RideConfigParams {
        venue: String::new(),
        ..params()
    });
    let mut fixture = fixture(
        config,
        FakeRoster::with_violations(two_entries(), violations),
    );

    // Act
    let result = fixture.engine.start(None);

    // Assert
    assert_eq!(
        result,
        Err(RideError::StartBlocked {
            message: "ride setup is incomplete: venue is required".to_owned(),
            reasons: vec!["venue is required".to_owned()],
        })
    );
}

#[test]
fn test_start_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let result = fixture.engine.start(None);

    // Assert
    assert_eq!(result, Err(illegal("cannot start from finished")));
}

#[test]
fn test_start_from_running_continues_and_keeps_actual_start() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.clock.advance(600);

    // Act
    let event = fixture.engine.start(None).expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.state(), RideStatus::Running);
    assert_eq!(
        event,
        Event {
            action: EventAction::Continue,
            payload: json!({
                "actual_start": "2026-09-20T10:00:00",
                "reason": "0:00:00",
            }),
        }
    );
    assert_eq!(fixture.engine.elapsed(), Ok(600.0));
}

#[test]
fn test_start_from_stopped_clears_the_stop_guard() {
    // Arrange
    let mut fixture = stopped_fixture();

    // Act
    fixture.engine.start(None).expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.stopped(), false);
    assert_eq!(
        fixture.engine.events().last().map(|event| event.action),
        Some(EventAction::Continue)
    );
}

#[test]
fn test_start_from_reopened_continues_and_keeps_actual_start() {
    // Arrange
    let mut fixture = reopened_fixture();

    // Act
    let event = fixture.engine.start(None).expect("the ride is reopened");

    // Assert
    assert_eq!(fixture.engine.state(), RideStatus::Running);
    assert_eq!(event.action, EventAction::Continue);
    assert_eq!(
        event.payload,
        json!({
            "actual_start": "2026-09-20T10:00:00",
            "reason": "0:00:00",
        })
    );
}

#[test]
fn test_start_from_reopened_discards_the_finished_instant() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.clock.advance(600);
    fixture.engine.finish(&[]).expect("the ride is running");
    fixture.engine.reopen().expect("the ride is finished");

    // Act
    fixture.engine.start(None).expect("the ride is reopened");

    // Assert
    assert_eq!(fixture.engine.closed_elapsed(), 0.0);
}

#[test]
fn test_start_from_reopened_sets_the_roster_status_to_running() {
    // Arrange
    let mut fixture = reopened_fixture();

    // Act
    fixture.engine.start(None).expect("the ride is reopened");

    // Assert
    assert_eq!(
        fixture.roster.statuses(),
        vec![
            RideStatus::Running,
            RideStatus::Finished,
            RideStatus::Reopened,
            RideStatus::Running,
        ]
    );
}

#[test]
fn test_start_continue_without_a_start_instant_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    fixture.engine.state = RideStatus::Running;

    // Act
    let result = fixture.engine.start(None);

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
}

// --------------------------------------------------------------- stop

#[test]
fn test_stop_from_running_marks_stopped_and_appends_the_stop_event() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.clock.advance(60);

    // Act
    let event = fixture.engine.stop().expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.stopped(), true);
    assert_eq!(fixture.engine.state(), RideStatus::Running);
    assert_eq!(
        event,
        Event {
            action: EventAction::Stop,
            payload: json!({
                "stopped_at": "2026-09-20T10:01:00",
                "reason": "0:01:00",
            }),
        }
    );
}

#[test]
fn test_stop_does_not_change_the_roster_status() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    fixture.engine.stop().expect("the ride is running");

    // Assert
    assert_eq!(fixture.roster.statuses(), vec![RideStatus::Running]);
}

#[test]
fn test_stop_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture.engine.stop();

    // Assert
    assert_eq!(result, Err(illegal("cannot stop a draft ride")));
}

#[test]
fn test_stop_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let result = fixture.engine.stop();

    // Assert
    assert_eq!(result, Err(illegal("cannot stop a finished ride")));
}

#[test]
fn test_stop_from_reopened_is_refused() {
    // Arrange
    let mut fixture = reopened_fixture();

    // Act
    let result = fixture.engine.stop();

    // Assert
    assert_eq!(result, Err(illegal("cannot stop a reopened ride")));
}

#[test]
fn test_stop_twice_is_refused() {
    // Arrange
    let mut fixture = stopped_fixture();

    // Act
    let result = fixture.engine.stop();

    // Assert
    assert_eq!(result, Err(illegal("ride is already stopped")));
}

#[test]
fn test_stop_without_a_start_instant_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    fixture.engine.state = RideStatus::Running;

    // Act
    let result = fixture.engine.stop();

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
    assert_eq!(fixture.engine.stopped(), false);
}

// ------------------------------------------------------------- finish

#[test]
fn test_finish_from_running_moves_to_finished_and_closes_the_shoe() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.clock.advance(1234);

    // Act
    let event = fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.state(), RideStatus::Finished);
    assert_eq!(
        event,
        Event {
            action: EventAction::Finish,
            payload: json!({
                "finished_at": "2026-09-20T10:20:34",
                "reason": "0:20:34",
            }),
        }
    );
    assert_eq!(fixture.engine.shoe_remaining(), 417);
    assert_eq!(fixture.engine.shoe_total(), 417);
}

#[test]
fn test_finish_from_reopened_moves_to_finished() {
    // Arrange
    let mut fixture = reopened_fixture();

    // Act
    fixture.engine.finish(&[]).expect("the ride is reopened");

    // Assert
    assert_eq!(fixture.engine.state(), RideStatus::Finished);
    assert_eq!(
        fixture
            .engine
            .events()
            .iter()
            .rev()
            .nth(1)
            .map(|event| event.action),
        Some(EventAction::Finish)
    );
}

#[test]
fn test_finish_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture.engine.finish(&[]);

    // Assert
    assert_eq!(result, Err(illegal("cannot finish from draft")));
}

#[test]
fn test_finish_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let result = fixture.engine.finish(&[]);

    // Assert
    assert_eq!(result, Err(illegal("cannot finish from finished")));
}

#[test]
fn test_finish_sets_the_roster_status_to_finished() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        fixture.roster.statuses(),
        vec![RideStatus::Running, RideStatus::Finished]
    );
}

#[test]
fn test_finish_clean_writes_no_self_test_key() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let event = fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        event.payload.get("self_test_failed_checks"),
        None,
        "a clean finish records no failed checks"
    );
}

#[test]
fn test_finish_with_failed_checks_writes_them_and_reads_unverified() {
    // Arrange
    let mut fixture = running_fixture();
    let checks = vec![
        "7,462 distinct ranks".to_owned(),
        "compare() total order".to_owned(),
    ];

    // Act
    let event = fixture.engine.finish(&checks).expect("the ride is running");

    // Assert
    assert_eq!(
        event.payload.get("self_test_failed_checks"),
        Some(&json!(["7,462 distinct ranks", "compare() total order"]))
    );
    assert_eq!(fixture.engine.self_test_unverified(), true);
}

#[test]
fn test_finish_clean_after_a_failed_finish_reads_verified() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .finish(&["7,462 distinct ranks".to_owned()])
        .expect("the ride is running");
    fixture.engine.reopen().expect("the ride is finished");

    // Act
    fixture.engine.finish(&[]).expect("the ride is reopened");

    // Assert
    assert_eq!(fixture.engine.self_test_unverified(), false);
}

#[test]
fn test_finish_without_a_start_instant_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    fixture.engine.state = RideStatus::Running;

    // Act
    let result = fixture.engine.finish(&[]);

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
}

// ------------------------------------------------- find the instant

#[test]
fn test_self_test_unverified_is_false_before_a_finish() {
    // Arrange
    let fixture = running_fixture();

    // Act / Assert
    assert_eq!(fixture.engine.self_test_unverified(), false);
}

#[test]
fn test_self_test_unverified_is_false_when_reopened() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .finish(&["7,462 distinct ranks".to_owned()])
        .expect("the ride is running");

    // Act
    fixture.engine.reopen().expect("the ride is finished");

    // Assert
    assert_eq!(fixture.engine.self_test_unverified(), false);
}

// ------------------------------------------------------------- reopen

#[test]
fn test_reopen_from_finished_moves_to_reopened_and_reopens_the_shoe() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let event = fixture.engine.reopen().expect("the ride is finished");

    // Assert
    assert_eq!(fixture.engine.state(), RideStatus::Reopened);
    assert_eq!(event.action, EventAction::Reopen);
    assert_eq!(
        fixture.engine.events().last().map(|event| event.action),
        Some(EventAction::Reopen)
    );
    assert_eq!(fixture.engine.shoe_remaining(), 417);
}

#[test]
fn test_reopen_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture.engine.reopen();

    // Assert
    assert_eq!(result, Err(illegal("cannot reopen from draft")));
}

#[test]
fn test_reopen_from_running_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.reopen();

    // Assert
    assert_eq!(result, Err(illegal("cannot reopen from running")));
}

#[test]
fn test_reopen_from_reopened_is_refused() {
    // Arrange
    let mut fixture = reopened_fixture();

    // Act
    let result = fixture.engine.reopen();

    // Assert
    assert_eq!(result, Err(illegal("cannot reopen from reopened")));
}

#[test]
fn test_reopen_sets_the_roster_status_to_reopened() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    fixture.engine.reopen().expect("the ride is finished");

    // Assert
    assert_eq!(
        fixture.roster.statuses(),
        vec![
            RideStatus::Running,
            RideStatus::Finished,
            RideStatus::Reopened,
        ]
    );
}

#[test]
fn test_reopen_without_a_start_instant_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    fixture.engine.state = RideStatus::Finished;

    // Act
    let result = fixture.engine.reopen();

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
}

// ---------------------------------------------------- set start time

#[test]
fn test_set_start_time_back_dates_and_logs_the_previous_start() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let event = fixture
        .engine
        .set_start_time(at(9, 55, 0))
        .expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.actual_start(), Some(at(9, 55, 0)));
    assert_eq!(
        event,
        Event {
            action: EventAction::SetStartTime,
            payload: json!({
                "actual_start": "2026-09-20T09:55:00",
                "previous_start": "2026-09-20T10:00:00",
                "reason": "-0:05:00",
            }),
        }
    );
}

#[test]
fn test_set_start_time_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture.engine.set_start_time(at(9, 55, 0));

    // Assert
    assert_eq!(result, Err(illegal("cannot set start time from draft")));
}

#[test]
fn test_set_start_time_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let result = fixture.engine.set_start_time(at(9, 55, 0));

    // Assert
    assert_eq!(result, Err(illegal("cannot set start time from finished")));
}

#[test]
fn test_set_start_time_from_reopened_is_refused() {
    // Arrange
    let mut fixture = reopened_fixture();

    // Act
    let result = fixture.engine.set_start_time(at(9, 55, 0));

    // Assert
    assert_eq!(result, Err(illegal("cannot set start time from reopened")));
}

#[test]
fn test_set_start_time_recomputes_lap_one() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 3, 20)));

    // Act
    fixture
        .engine
        .set_start_time(at(9, 55, 0))
        .expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.lap_times(&entry), vec![500.0]);
}

#[test]
fn test_set_start_time_leaves_later_laps() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 3, 20)));
    let _ = fixture.engine.record_crossing("12", Some(at(10, 5, 0)));

    // Act
    fixture
        .engine
        .set_start_time(at(9, 55, 0))
        .expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.lap_times(&entry), vec![500.0, 100.0]);
}

#[test]
fn test_set_start_time_without_a_start_instant_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    fixture.engine.state = RideStatus::Running;

    // Act
    let result = fixture.engine.set_start_time(at(9, 55, 0));

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
}

// ------------------------------------------------------------- timing

#[test]
fn test_elapsed_derives_from_the_clock() {
    // Arrange
    let fixture = running_fixture();

    // Act
    fixture.clock.advance(90);

    // Assert
    assert_eq!(fixture.engine.elapsed(), Ok(90.0));
}

#[test]
fn test_elapsed_before_the_start_is_refused() {
    // Arrange
    let fixture = default_fixture();

    // Act
    let result = fixture.engine.elapsed();

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
}

#[test]
fn test_elapsed_after_finish_keeps_deriving_from_the_clock() {
    // Arrange
    let fixture = finished_fixture();

    // Act
    fixture.clock.advance(120);

    // Assert
    assert_eq!(fixture.engine.elapsed(), Ok(120.0));
}

#[test]
fn test_stop_does_not_freeze_elapsed() {
    // Arrange
    let fixture = stopped_fixture();

    // Act
    fixture.clock.advance(60);

    // Assert
    assert_eq!(fixture.engine.elapsed(), Ok(60.0));
}

#[test]
fn test_remaining_derives_from_the_clock() {
    // Arrange
    let fixture = running_fixture();

    // Act
    fixture.clock.advance(60);

    // Assert
    assert_eq!(fixture.engine.remaining(), Ok(21_540.0));
}

#[test]
fn test_remaining_before_the_start_is_refused() {
    // Arrange
    let fixture = default_fixture();

    // Act
    let result = fixture.engine.remaining();

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
}

#[test]
fn test_closed_elapsed_on_a_finished_ride_reports_the_recorded_finish() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.clock.advance(1234);

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.closed_elapsed(), 1234.0);
}

#[test]
fn test_closed_elapsed_on_a_reopened_ride_keeps_the_finish_elapsed() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.clock.advance(500);
    fixture.engine.finish(&[]).expect("the ride is running");
    fixture.clock.advance(9999);

    // Act
    fixture.engine.reopen().expect("the ride is finished");

    // Assert
    assert_eq!(fixture.engine.closed_elapsed(), 500.0);
}

#[test]
fn test_closed_elapsed_before_the_start_is_zero() {
    // Arrange
    let fixture = default_fixture();

    // Act / Assert
    assert_eq!(fixture.engine.closed_elapsed(), 0.0);
}

#[test]
fn test_closed_elapsed_without_a_start_instant_is_zero() {
    // Arrange
    let mut fixture = default_fixture();
    fixture.engine.state = RideStatus::Finished;
    fixture.engine.finished_at = Some(at(11, 0, 0));

    // Act / Assert
    assert_eq!(fixture.engine.closed_elapsed(), 0.0);
}

// ------------------------------------------------------ record crossing

#[test]
fn test_record_crossing_before_the_start_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture.engine.record_crossing("12", None);

    // Assert
    assert_eq!(result, refused("12", "ride is not running"));
}

#[test]
fn test_record_crossing_after_finish_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let result = fixture.engine.record_crossing("12", None);

    // Assert
    assert_eq!(result, refused("12", "ride is not running"));
}

#[test]
fn test_record_crossing_while_stopped_is_refused() {
    // Arrange
    let mut fixture = stopped_fixture();

    // Act
    let result = fixture.engine.record_crossing("12", None);

    // Assert
    assert_eq!(result, refused("12", "ride is stopped"));
    assert_eq!(fixture.engine.crossings(), Vec::new());
}

#[test]
fn test_record_crossing_with_an_unknown_plate_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.record_crossing("999", None);

    // Assert
    assert_eq!(result, refused("999", "unknown_plate"));
    assert_eq!(fixture.engine.crossings(), Vec::new());
}

#[test]
fn test_record_crossing_without_a_start_instant_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    fixture.engine.state = RideStatus::Running;

    // Act
    let result = fixture.engine.record_crossing("12", None);

    // Assert
    assert_eq!(result, refused("12", "ride has not started"));
}

#[test]
fn test_record_crossing_credits_lap_one_from_the_actual_start() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let expected_card = reference_card(fixture.engine.config(), 0);

    // Act
    let result = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));

    // Assert
    assert_eq!(
        (result.accepted, result.lap, result.lap_time),
        (true, 1, 120.0)
    );
    assert_eq!(result.entry_plate.as_deref(), Some("12"));
    assert_eq!(result.entry_name.as_deref(), Some("Rider 12"));
    assert_eq!(result.card, Some(expected_card));
    assert_eq!(result.flagged, false);
}

#[test]
fn test_record_crossing_times_lap_two_from_the_previous_crossing() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));

    // Act
    let result = fixture.engine.record_crossing("12", Some(at(10, 4, 30)));

    // Assert
    assert_eq!((result.lap, result.lap_time), (2, 150.0));
}

#[test]
fn test_record_crossing_uses_the_clock_when_the_instant_is_omitted() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.clock.advance(90);

    // Act
    let result = fixture.engine.record_crossing("12", None);

    // Assert
    assert_eq!((result.accepted, result.lap_time), (true, 90.0));
    assert_eq!(
        result.card,
        Some(reference_card(fixture.engine.config(), 0))
    );
}

#[test]
fn test_record_crossing_marks_the_entry_has_data() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));

    // Assert
    assert_eq!(fixture.roster.has_data(), vec![entry]);
}

#[test]
fn test_record_crossing_appends_the_event_with_the_reason() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));

    // Assert
    assert_eq!(
        fixture.engine.events().last(),
        Some(&Event {
            action: EventAction::RecordCrossing,
            payload: json!({
                "plate": "12",
                "entry_id": "key-12",
                "lap": 1,
                "crossed_at": "2026-09-20T10:02:00",
                "reason": "Rider 12 · solo",
            }),
        })
    );
}

#[test]
fn test_record_crossing_stores_the_typed_plate_on_the_crossing() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));

    // Assert
    assert_eq!(fixture.engine.crossings().len(), 1);
    assert_eq!(
        fixture.engine.crossings().first(),
        Some(&Crossing {
            entry_id: entry,
            seq: 1,
            crossed_at: at(10, 2, 0),
            rider_plate: Some("12".to_owned()),
            deal_seq: 0,
        })
    );
}

#[test]
fn test_record_crossing_pooled_rider_plate_credits_the_team() {
    // Arrange
    let mut fixture = fixture(fixture_config(), FakeRoster::new(vec![pooled_team()]));
    fixture.engine.start(None).expect("the roster is ready");

    // Act
    let result = fixture.engine.record_crossing("45", Some(at(10, 2, 0)));

    // Assert
    assert_eq!(result.accepted, true);
    assert_eq!(result.entry_plate.as_deref(), Some("9"));
    assert_eq!(result.entry_name.as_deref(), Some("Dirt Dynamos"));
}

#[test]
fn test_record_crossing_reason_for_a_solo_entry() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));
    let event = fixture.engine.events().last();

    // Assert
    assert_eq!(
        event.and_then(|recorded| recorded.payload.get("reason")),
        Some(&json!("Rider 12 · solo"))
    );
}

#[test]
fn test_record_crossing_reason_for_a_pooled_team() {
    // Arrange
    let mut fixture = fixture(fixture_config(), FakeRoster::new(vec![pooled_team()]));
    fixture.engine.start(None).expect("the roster is ready");

    // Act
    let _ = fixture.engine.record_crossing("45", Some(at(10, 2, 0)));
    let event = fixture.engine.events().last();

    // Assert
    assert_eq!(
        event.and_then(|recorded| recorded.payload.get("reason")),
        Some(&json!("Sarah · Dirt Dynamos"))
    );
}

#[test]
fn test_record_crossing_reason_for_a_relay_team() {
    // Arrange
    let mut fixture = fixture(fixture_config(), FakeRoster::new(vec![relay_team()]));
    fixture.engine.start(None).expect("the roster is ready");

    // Act
    let _ = fixture.engine.record_crossing("77", Some(at(10, 2, 0)));
    let event = fixture.engine.events().last();

    // Assert
    assert_eq!(
        event.and_then(|recorded| recorded.payload.get("reason")),
        Some(&json!("77 · Road Crew"))
    );
}

#[test]
fn test_record_crossing_reason_falls_back_when_the_rider_name_is_blank() {
    // Arrange
    let mut entry = solo_entry("12");
    entry.riders = vec![RiderSnapshot {
        plate: Some("12".to_owned()),
        full_name: String::new(),
        sex: None,
    }];
    let mut fixture = fixture(fixture_config(), FakeRoster::new(vec![entry]));
    fixture.engine.start(None).expect("the roster is ready");

    // Act
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));
    let event = fixture.engine.events().last();

    // Assert
    assert_eq!(
        event.and_then(|recorded| recorded.payload.get("reason")),
        Some(&json!("12 · solo"))
    );
}

#[test]
fn test_record_crossing_deals_the_next_shoe_card_in_order() {
    // Arrange
    let mut fixture = running_fixture();
    let config = fixture.engine.config().clone();
    let first_card = reference_card(&config, 0);
    let second_card = reference_card(&config, 1);
    let third_card = reference_card(&config, 2);

    // Act
    let first = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let second = fixture.engine.record_crossing("12", Some(at(10, 32, 0)));
    let third = fixture.engine.record_crossing("34", Some(at(10, 34, 0)));

    // Assert
    assert_eq!(first.card, Some(first_card));
    assert_eq!(second.card, Some(second_card));
    assert_eq!(third.card, Some(third_card));
    assert_eq!(fixture.engine.shoe_remaining(), 414);
}

#[test]
fn test_record_crossing_at_the_exact_minimum_lap_is_not_flagged() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.record_crossing("12", Some(at(10, 18, 0)));

    // Assert
    assert_eq!((result.flagged, result.lap_time), (false, 1080.0));
    assert_eq!(fixture.engine.held_crossings(), Vec::new());
}

#[test]
fn test_record_crossing_one_second_under_the_minimum_is_flagged_and_held() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.record_crossing("12", Some(at(10, 17, 59)));

    // Assert
    assert_eq!((result.flagged, result.lap_time), (true, 1079.0));
    assert_eq!(
        fixture.engine.held_crossings(),
        vec![HeldCrossing {
            crossing: fixture
                .engine
                .crossings()
                .first()
                .cloned()
                .expect("the short lap is recorded"),
            card: result.card.clone().expect("the short lap deals a card"),
        }]
    );
}

#[test]
fn test_record_crossing_short_lap_still_records_the_lap() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));

    // Assert
    assert_eq!(fixture.engine.crossings().len(), 1);
    assert_eq!(fixture.engine.lap_times(&entry), vec![30.0]);
}

#[test]
fn test_record_crossing_short_lap_under_hold_leaves_the_hand_empty() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));

    // Assert
    assert_eq!(fixture.engine.credited_cards(&entry), Vec::new());
    assert_eq!(fixture.engine.held_crossings().len(), 1);
}

#[test]
fn test_record_crossing_short_lap_under_always_deal_flags_and_credits() {
    // Arrange
    let config = config_with(RideConfigParams {
        hold_short_laps: false,
        min_lap_s: 60,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");

    // Act
    let result = fixture.engine.record_crossing("12", Some(at(10, 0, 59)));

    // Assert
    assert_eq!((result.flagged, result.lap_time), (true, 59.0));
    assert_eq!(fixture.engine.held_crossings(), Vec::new());
    assert_eq!(
        fixture.engine.credited_cards(&entry),
        vec![result.card.clone().expect("the short lap deals a card"),]
    );
}

#[test]
fn test_record_crossing_short_lap_under_hold_flags_at_the_boundary() {
    // Arrange
    let config = config_with(RideConfigParams {
        hold_short_laps: true,
        min_lap_s: 60,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");

    // Act
    let result = fixture.engine.record_crossing("12", Some(at(10, 0, 59)));

    // Assert
    assert_eq!((result.flagged, result.lap_time), (true, 59.0));
    assert_eq!(fixture.engine.credited_cards(&entry), Vec::new());
    assert_eq!(fixture.engine.held_crossings().len(), 1);
}

#[test]
fn test_record_crossing_always_deal_credits_every_accepted_lap() {
    // Arrange
    let config = config_with(RideConfigParams {
        hold_short_laps: false,
        min_lap_s: 600,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");

    // Act
    let first = fixture.engine.record_crossing("12", Some(at(10, 0, 5)));
    let second = fixture.engine.record_crossing("12", Some(at(10, 10, 4)));
    let third = fixture.engine.record_crossing("12", Some(at(10, 20, 4)));

    // Assert
    assert_eq!(
        (
            first.flagged,
            second.flagged,
            third.flagged,
            fixture.engine.held_crossings().len(),
        ),
        (true, true, false, 0)
    );
    assert_eq!(fixture.engine.credited_cards(&entry).len(), 3);
}

#[test]
fn test_record_crossing_pooled_rider_out_laps_teammates_uncapped() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 60,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(vec![pooled_team()]));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "9");

    // Act
    record_many(&mut fixture.engine, "45", 5, at(10, 1, 0), 600);
    record_many(&mut fixture.engine, "9", 2, at(10, 51, 0), 600);

    // Assert
    assert_eq!(fixture.engine.crossings().len(), 7);
    assert_eq!(fixture.engine.credited_cards(&entry).len(), 7);
    assert_eq!(
        fixture.engine.lap_times(&entry),
        vec![60.0, 600.0, 600.0, 600.0, 600.0, 600.0, 600.0]
    );
}

#[test]
fn test_record_crossing_reshuffles_and_audits_when_the_shoe_empties() {
    // Arrange
    let config = config_with(RideConfigParams {
        deck_count: 1,
        jokers_per_deck: 0,
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    record_many(&mut fixture.engine, "12", 52, at(10, 0, 0), 60);

    // Act
    let result = fixture.engine.record_crossing("12", Some(at(10, 52, 0)));

    // Assert
    assert_eq!(result.accepted, true);
    assert_eq!(
        fixture
            .engine
            .events()
            .get(fixture.engine.events().len() - 2),
        Some(&Event {
            action: EventAction::ShoeReshuffle,
            payload: json!({
                "cycle": 2,
                "jokers_added": 0,
                "reason": "0 jokers added",
            }),
        })
    );
    assert_eq!(
        fixture.engine.events().last().map(|event| event.action),
        Some(EventAction::RecordCrossing)
    );
    let mut reshuffled = Shoe::new(1, 0, SEED + 1, JokersMode::PerDeck).expect("the config valid");
    assert_eq!(
        result.card,
        Some(reshuffled.deal().expect("the reshuffled shoe deals").0)
    );
}

#[test]
fn test_record_crossing_reshuffle_reports_the_jokers_added() {
    // Arrange
    let config = config_with(RideConfigParams {
        deck_count: 1,
        jokers_per_deck: 2,
        jokers_mode: "per_deck".to_owned(),
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    record_many(&mut fixture.engine, "12", 54, at(10, 0, 0), 60);

    // Act
    let _ = fixture.engine.record_crossing("12", Some(at(10, 54, 0)));

    // Assert
    assert_eq!(
        fixture
            .engine
            .events()
            .get(fixture.engine.events().len() - 2),
        Some(&Event {
            action: EventAction::ShoeReshuffle,
            payload: json!({
                "cycle": 2,
                "jokers_added": 2,
                "reason": "2 jokers added",
            }),
        })
    );
}

#[test]
fn test_record_crossing_with_a_closed_shoe_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.engine.shoe.close();

    // Act
    let result = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));

    // Assert
    assert_eq!(result, refused("12", "shoe is closed"));
    assert_eq!(fixture.engine.crossings(), Vec::new());
}

#[test]
fn test_record_crossing_with_a_drained_closed_shoe_is_refused() {
    // Arrange
    let config = config_with(RideConfigParams {
        deck_count: 1,
        jokers_per_deck: 0,
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    record_many(&mut fixture.engine, "12", 52, at(10, 0, 0), 60);
    fixture.engine.shoe.close();

    // Act
    let result = fixture.engine.record_crossing("12", Some(at(10, 52, 0)));

    // Assert
    assert_eq!(result, refused("12", "shoe is closed"));
    assert_eq!(fixture.engine.crossings().len(), 52);
    assert_eq!(
        fixture.engine.events().last().map(|event| event.action),
        Some(EventAction::RecordCrossing)
    );
}

// ------------------------------------------------------------- queries

#[test]
fn test_crossings_returns_every_crossing_oldest_first() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));
    let _ = fixture.engine.record_crossing("34", Some(at(10, 3, 0)));
    let _ = fixture.engine.record_crossing("12", Some(at(10, 4, 0)));

    // Act
    let recorded: Vec<(EntryId, u32)> = fixture
        .engine
        .crossings()
        .iter()
        .map(|crossing| (crossing.entry_id.clone(), crossing.seq))
        .collect();

    // Assert
    assert_eq!(
        recorded,
        vec![
            (EntryId("key-12".to_owned()), 1),
            (EntryId("key-34".to_owned()), 1),
            (EntryId("key-12".to_owned()), 2),
        ]
    );
}

#[test]
fn test_lap_times_before_the_start_are_empty() {
    // Arrange
    let fixture = default_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let times = fixture.engine.lap_times(&entry);

    // Assert
    assert_eq!(times, Vec::<f64>::new());
}

#[test]
fn test_lap_times_for_an_entry_without_laps_are_empty() {
    // Arrange
    let fixture = running_fixture();
    let entry = key(&fixture.roster, "34");

    // Act
    let times = fixture.engine.lap_times(&entry);

    // Assert
    assert_eq!(times, Vec::<f64>::new());
}

#[test]
fn test_on_course_counts_active_entries_with_an_odd_lap_count() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));
    let _ = fixture.engine.record_crossing("34", Some(at(10, 3, 0)));
    let _ = fixture.engine.record_crossing("34", Some(at(10, 4, 0)));

    // Act / Assert
    assert_eq!(fixture.engine.on_course(), 1);
}

#[test]
fn test_on_course_excludes_a_dnf_entry() {
    // Arrange
    let mut dnf = solo_entry("12");
    dnf.is_dnf = true;
    let mut fixture = fixture(fixture_config(), FakeRoster::new(vec![dnf]));
    fixture.engine.start(None).expect("the roster is ready");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));

    // Act / Assert
    assert_eq!(fixture.engine.on_course(), 0);
}

#[test]
fn test_entry_count_reads_the_live_roster() {
    // Arrange
    let fixture = default_fixture();
    let count_before = fixture.engine.entry_count();

    // Act
    fixture.roster.add_entry(solo_entry("56"));

    // Assert
    assert_eq!(count_before, 2);
    assert_eq!(fixture.engine.entry_count(), 3);
}

#[test]
fn test_shoe_remaining_and_total_track_the_current_cycle() {
    // Arrange
    let fixture = default_fixture();

    // Act / Assert
    assert_eq!(fixture.engine.shoe_total(), 417);
    assert_eq!(fixture.engine.shoe_remaining(), 417);
}

#[test]
fn test_events_returns_every_event_oldest_first() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));

    // Act
    let actions: Vec<EventAction> = fixture
        .engine
        .events()
        .iter()
        .map(|event| event.action)
        .collect();

    // Assert
    assert_eq!(
        actions,
        vec![EventAction::Start, EventAction::RecordCrossing]
    );
}

#[test]
fn test_held_crossings_list_the_held_card_oldest_first() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let _ = fixture.engine.record_crossing("34", Some(at(10, 0, 40)));

    // Act
    let held = fixture.engine.held_crossings();

    // Assert
    assert_eq!(held.len(), 2);
    assert_eq!(held.first().map(|entry| entry.crossing.seq), Some(1));
    assert_eq!(
        held.iter()
            .map(|entry| entry.crossing.entry_id.clone())
            .collect::<Vec<EntryId>>(),
        vec![EntryId("key-12".to_owned()), EntryId("key-34".to_owned())]
    );
}

#[test]
fn test_credited_cards_list_the_dealt_cards_in_deal_order() {
    // Arrange
    let config = config_with(RideConfigParams {
        hold_short_laps: false,
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");

    // Act
    let first = fixture.engine.record_crossing("12", Some(at(10, 1, 0)));
    let second = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));

    // Assert
    assert_eq!(
        fixture.engine.credited_cards(&entry),
        vec![
            first.card.expect("the first lap deals a card"),
            second.card.expect("the second lap deals a card"),
        ]
    );
}

#[test]
fn test_credited_cards_for_an_unknown_entry_are_empty() {
    // Arrange
    let fixture = running_fixture();

    // Act
    let cards = fixture
        .engine
        .credited_cards(&EntryId("key-missing".to_owned()));

    // Assert
    assert_eq!(cards, Vec::new());
}

// --------------------------------------------------------- laps index

#[test]
fn test_laps_index_sorts_per_entry_by_crossing_instant() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let late = fixture.engine.record_crossing("12", Some(at(10, 10, 0)));
    let early = fixture.engine.record_crossing("12", Some(at(10, 5, 0)));

    // Assert
    assert_eq!(late.lap_time, 600.0);
    assert_eq!(early.lap_time, -300.0);
    assert_eq!(fixture.engine.lap_times(&entry), vec![300.0, 300.0]);
}

#[test]
fn test_laps_index_keeps_record_order_for_equal_instants() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let first = fixture.engine.record_crossing("12", Some(at(10, 5, 0)));
    let second = fixture.engine.record_crossing("12", Some(at(10, 5, 0)));

    // Assert
    assert_eq!((first.lap, second.lap), (1, 2));
    assert_eq!(fixture.engine.lap_times(&entry), vec![300.0, 0.0]);
    let seqs: Vec<u32> = fixture
        .engine
        .crossings()
        .iter()
        .map(|crossing| crossing.seq)
        .collect();
    assert_eq!(seqs, vec![1, 2]);
}

#[test]
fn test_laps_index_identifies_each_entry_separately() {
    // Arrange
    let mut fixture = running_fixture();
    let first = key(&fixture.roster, "12");
    let second = key(&fixture.roster, "34");

    // Act
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));
    let _ = fixture.engine.record_crossing("34", Some(at(10, 5, 0)));

    // Assert
    assert_eq!(fixture.engine.lap_times(&first), vec![120.0]);
    assert_eq!(fixture.engine.lap_times(&second), vec![300.0]);
}

#[test]
fn test_laps_index_matches_a_sorted_replay() {
    // Arrange
    let mut out_of_order = running_fixture();
    let mut in_order = running_fixture();
    let out_of_order_key = key(&out_of_order.roster, "12");
    let in_order_key = key(&in_order.roster, "12");

    // Act
    let _ = out_of_order
        .engine
        .record_crossing("12", Some(at(10, 5, 0)));
    let _ = out_of_order
        .engine
        .record_crossing("12", Some(at(10, 3, 0)));
    let _ = in_order.engine.record_crossing("12", Some(at(10, 3, 0)));
    let _ = in_order.engine.record_crossing("12", Some(at(10, 5, 0)));

    // Assert
    assert_eq!(
        out_of_order.engine.lap_times(&out_of_order_key),
        in_order.engine.lap_times(&in_order_key)
    );
    assert_eq!(
        out_of_order.engine.lap_times(&out_of_order_key),
        vec![180.0, 120.0]
    );
}

// ------------------------------------------------------- update config

#[test]
fn test_update_config_replaces_the_live_config() {
    // Arrange
    let mut fixture = default_fixture();
    let edited = config_with(RideConfigParams {
        name: "Renamed Ride".to_owned(),
        venue: "New Venue".to_owned(),
        lap_km: 6.5,
        ..params()
    });

    // Act
    fixture
        .engine
        .update_config(edited.clone())
        .expect("the config is valid");

    // Assert
    assert_eq!(fixture.engine.config(), &edited);
}

#[test]
fn test_update_config_keeps_the_ride_state_and_its_events() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));
    let events_before = fixture.engine.events().to_vec();
    let crossings_before = fixture.engine.crossings().to_vec();
    let edited = config_with(RideConfigParams {
        name: "Renamed Ride".to_owned(),
        ..params()
    });

    // Act
    fixture
        .engine
        .update_config(edited)
        .expect("the config is valid");

    // Assert
    assert_eq!(fixture.engine.state(), RideStatus::Running);
    assert_eq!(fixture.engine.events(), events_before.as_slice());
    assert_eq!(fixture.engine.crossings(), crossings_before.as_slice());
}

#[test]
fn test_update_config_in_draft_with_a_deck_change_rebuilds_the_shoe() {
    // Arrange
    let config = config_with(RideConfigParams {
        jokers_mode: "per_deck".to_owned(),
        deck_count: 8,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));

    // Act
    fixture
        .engine
        .update_config(config_with(RideConfigParams {
            jokers_mode: "per_deck".to_owned(),
            deck_count: 2,
            ..params()
        }))
        .expect("the config is valid");

    // Assert
    assert_eq!(fixture.engine.shoe_total(), 106);
    assert_eq!(fixture.engine.shoe_remaining(), 106);
}

#[test]
fn test_update_config_in_draft_with_a_joker_change_re_deals_from_the_seed() {
    // Arrange
    let config = config_with(RideConfigParams {
        jokers_mode: "per_deck".to_owned(),
        deck_count: 8,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    let edited = config_with(RideConfigParams {
        jokers_mode: "per_deck".to_owned(),
        deck_count: 2,
        jokers_per_deck: 2,
        ..params()
    });

    // Act
    fixture
        .engine
        .update_config(edited)
        .expect("the config is valid");
    fixture.engine.start(None).expect("the roster is ready");
    let result = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let mut reference =
        Shoe::new(2, 2, SEED, JokersMode::PerDeck).expect("the reference config is valid");

    // Assert
    assert_eq!(fixture.engine.shoe_total(), 108);
    assert_eq!(
        result.card,
        Some(reference.deal().expect("the reference deals").0)
    );
}

#[test]
fn test_update_config_in_draft_with_a_mode_change_rebuilds_the_shoe() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    fixture
        .engine
        .update_config(config_with(RideConfigParams {
            jokers_mode: "per_deck".to_owned(),
            ..params()
        }))
        .expect("the config is valid");

    // Assert
    assert_eq!(fixture.engine.shoe_total(), 424);
}

#[test]
fn test_update_config_in_draft_with_an_unchanged_structure_keeps_the_shoe() {
    // Arrange
    let mut fixture = default_fixture();
    let _first = fixture.engine.shoe.deal().expect("the shoe deals");
    let remaining_before = fixture.engine.shoe_remaining();
    let edited = config_with(RideConfigParams {
        name: "Renamed Ride".to_owned(),
        ..params()
    });

    // Act
    fixture
        .engine
        .update_config(edited)
        .expect("the config is valid");

    // Assert
    assert_eq!(fixture.engine.shoe_remaining(), remaining_before);
}

#[test]
fn test_update_config_in_running_keeps_the_live_shoe() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));
    let remaining_before = fixture.engine.shoe_remaining();

    // Act
    fixture
        .engine
        .update_config(config_with(RideConfigParams {
            jokers_mode: "per_deck".to_owned(),
            deck_count: 2,
            ..params()
        }))
        .expect("the config is valid");

    // Assert
    assert_eq!(fixture.engine.shoe_remaining(), remaining_before);
}

#[test]
fn test_update_config_in_finished_keeps_the_closed_shoe() {
    // Arrange
    let mut fixture = finished_fixture();
    let remaining_before = fixture.engine.shoe_remaining();

    // Act
    fixture
        .engine
        .update_config(config_with(RideConfigParams {
            jokers_mode: "per_deck".to_owned(),
            deck_count: 2,
            ..params()
        }))
        .expect("the config is valid");

    // Assert
    assert_eq!(fixture.engine.shoe_remaining(), remaining_before);
    assert_eq!(fixture.engine.state(), RideStatus::Finished);
}

#[test]
fn test_update_config_with_an_oversized_deck_count_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    let config_before = fixture.engine.config().clone();
    let mut edited = config_with(params());
    edited.deck_count = 300;

    // Act
    let result = fixture.engine.update_config(edited);

    // Assert
    assert_eq!(
        result,
        Err(RideError::Config {
            message: "deck_count must be at most 255, got 300".to_owned(),
        })
    );
    assert_eq!(fixture.engine.config(), &config_before);
}

#[test]
fn test_update_config_with_an_oversized_joker_count_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    let mut edited = config_with(params());
    edited.jokers_per_deck = 300;

    // Act
    let result = fixture.engine.update_config(edited);

    // Assert
    assert_eq!(
        result,
        Err(RideError::Config {
            message: "jokers_per_deck must be at most 255, got 300".to_owned(),
        })
    );
}

#[test]
fn test_update_config_with_a_closed_draft_shoe_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    fixture.engine.shoe.close();
    let config_before = fixture.engine.config().clone();

    // Act
    let result = fixture.engine.update_config(config_with(RideConfigParams {
        deck_count: 2,
        ..params()
    }));

    // Assert
    assert_eq!(result, Err(illegal("shoe is closed")));
    assert_eq!(fixture.engine.config(), &config_before);
}

// --------------------------------------------------------- performance

#[test]
fn test_record_crossing_batch_of_100_stays_within_a_generous_bound() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let started = Instant::now();
    record_many(&mut fixture.engine, "12", 100, at(10, 0, 0), 60);
    let elapsed = started.elapsed();

    // Assert
    assert_eq!(fixture.engine.crossings().len(), 100);
    assert!(
        elapsed.as_secs_f64() / 100.0 < 0.1,
        "recording averaged {} ms per crossing",
        elapsed.as_secs_f64() * 1000.0 / 100.0
    );
}

// ------------------------------------------------------- undo (R-33)

/// The seed whose two-deck per-deck shoe deals one code on deals six and
/// seven, so the ride's sixth and seventh crossings carry a same-code pair.
const IDENTITY_SEED: u64 = 20_260_901;

fn identity_config(min_lap_s: i32) -> RideConfig {
    config_with(RideConfigParams {
        deck_count: 2,
        jokers_per_deck: 1,
        jokers_mode: "per_deck".to_owned(),
        min_lap_s,
        ..params()
    })
}

fn identity_fixture(min_lap_s: i32) -> Fixture {
    fixture_with_seed(
        identity_config(min_lap_s),
        FakeRoster::new(two_entries()),
        IDENTITY_SEED,
    )
}

/// Record six crossings two seconds apart, then a seventh at `second_at`.
///
/// The sixth and seventh deals are the same code on two deal slots, so the
/// returned crossings are the identity pair every alias test starts from.
fn record_identity_pair(fixture: &mut Fixture, second_at: Timestamp) -> (Crossing, Crossing) {
    for step in 0..6_i64 {
        let instant = at(10, 0, 2) + Duration::seconds(step * 2);
        let _ = fixture.engine.record_crossing("12", Some(instant));
    }
    let _ = fixture.engine.record_crossing("12", Some(second_at));
    let crossings = fixture.engine.crossings();
    let first = crossings
        .get(5)
        .expect("the sixth crossing is recorded")
        .clone();
    let second = crossings
        .get(6)
        .expect("the seventh crossing is recorded")
        .clone();
    (first, second)
}

#[test]
fn test_undo_last_removes_the_lap_and_restitutes_the_card() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");
    let first = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let second = fixture.engine.record_crossing("12", Some(at(10, 32, 0)));
    let second_card = second.card.clone().expect("the second lap deals a card");

    // Act
    let event = fixture.engine.undo_last().expect("the ride has a crossing");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::Undo,
            payload: json!({
                "entry_id": "key-12",
                "seq": 2,
                "crossed_at": "2026-09-20T10:32:00",
                "card": second_card.code(),
                "reason": "Undo last crossing",
            }),
        }
    );
    assert_eq!(fixture.engine.crossings().len(), 1);
    assert_eq!(fixture.engine.lap_times(&entry), vec![1800.0]);
    assert_eq!(
        fixture.engine.credited_cards(&entry),
        vec![first.card.expect("the first lap deals a card")]
    );
}

#[test]
fn test_undo_last_restitutes_the_card_at_the_shoe_front() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");
    let original = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));

    // Act
    fixture.engine.undo_last().expect("the ride has a crossing");
    let rerecord = fixture.engine.record_crossing("12", Some(at(10, 31, 0)));

    // Assert
    assert_eq!(rerecord.card, original.card);
    assert_eq!(fixture.engine.lap_times(&entry), vec![1860.0]);
    assert_eq!(fixture.engine.shoe.dealt(), 1);
}

#[test]
fn test_undo_last_held_crossing_releases_the_hold_without_crediting() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let recorded = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");

    // Act
    fixture.engine.undo_last().expect("the ride has a crossing");

    // Assert
    assert_eq!(fixture.engine.held_crossings(), Vec::new());
    assert_eq!(fixture.engine.credited_cards(&entry), Vec::new());
    assert_eq!(fixture.engine.lap_times(&entry), Vec::<f64>::new());
    let redo = fixture.engine.record_crossing("12", Some(at(10, 0, 45)));
    assert_eq!(redo.card, Some(held.card));
    assert_eq!(redo.card, recorded.card);
}

#[test]
fn test_undo_last_voided_crossing_unmarks_the_void_and_returns_the_card() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");
    fixture
        .engine
        .void_held(&held.crossing)
        .expect("the card is held");
    assert!(fixture.engine.is_card_voided(&held.crossing));

    // Act
    fixture.engine.undo_last().expect("the ride has a crossing");

    // Assert
    assert!(!fixture.engine.is_card_voided(&held.crossing));
    assert_eq!(fixture.engine.crossings(), Vec::new());
    let redo = fixture.engine.record_crossing("12", Some(at(10, 0, 45)));
    assert_eq!(redo.card, Some(held.card));
}

#[test]
fn test_undo_last_with_a_closed_shoe_retires_the_card_without_error() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let recorded = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let card = recorded.card.expect("the lap deals a card");
    fixture.engine.shoe.close();

    // Act
    let event = fixture
        .engine
        .undo_last()
        .expect("a closed shoe suppresses restitution");

    // Assert
    assert_eq!(event.payload.get("card"), Some(&json!(card.code())));
    assert_eq!(fixture.engine.crossings(), Vec::new());
    assert_eq!(fixture.engine.shoe.dealt(), 1);
}

#[test]
fn test_undo_last_after_a_later_deal_retires_the_card() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let recorded = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let card = recorded.card.expect("the lap deals a card");
    let _later = fixture
        .engine
        .shoe
        .deal()
        .expect("the shoe has a later card at the front");

    // Act
    let event = fixture
        .engine
        .undo_last()
        .expect("a later front card suppresses restitution");

    // Assert
    assert_eq!(event.payload.get("card"), Some(&json!(card.code())));
    assert_eq!(fixture.engine.crossings(), Vec::new());
    assert_eq!(fixture.engine.shoe.dealt(), 2);
}

#[test]
fn test_undo_last_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture.engine.undo_last();

    // Assert
    assert_eq!(result, Err(illegal("cannot undo from draft")));
}

#[test]
fn test_undo_last_from_finished_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));
    fixture.engine.finish(&[]).expect("the ride is running");

    // Act
    let result = fixture.engine.undo_last();

    // Assert
    assert_eq!(result, Err(illegal("cannot undo from finished")));
}

#[test]
fn test_undo_last_with_no_crossings_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.undo_last();

    // Assert
    assert_eq!(result, Err(illegal("no crossings to undo")));
}

#[test]
fn test_undo_last_with_a_crossing_that_has_no_dealt_card_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));
    fixture.engine.dealt.clear();

    // Act
    let result = fixture.engine.undo_last();

    // Assert
    assert_eq!(result, Err(illegal("last crossing has no dealt card")));
    assert_eq!(fixture.engine.crossings().len(), 1);
}

#[test]
fn test_undo_last_from_reopened_reverses_the_crossing() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    fixture.engine.finish(&[]).expect("the ride is running");
    fixture.engine.reopen().expect("the ride is finished");

    // Act
    fixture.engine.undo_last().expect("the ride is reopened");

    // Assert
    assert_eq!(fixture.engine.lap_times(&entry), Vec::<f64>::new());
    assert_eq!(fixture.engine.crossings(), Vec::new());
}

#[test]
fn test_undo_last_from_reopened_returns_the_card_to_the_open_shoe() {
    // Arrange
    let mut fixture = running_fixture();
    let recorded = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let card = recorded.card.expect("the lap deals a card");
    fixture.engine.finish(&[]).expect("the ride is running");
    fixture.engine.reopen().expect("the ride is finished");

    // Act
    fixture.engine.undo_last().expect("the ride is reopened");

    // Assert
    assert_eq!(fixture.engine.shoe.dealt(), 0);
    let next = fixture
        .engine
        .shoe
        .deal()
        .expect("the reopened shoe deals")
        .0;
    assert_eq!(next, card);
}

#[test]
fn test_undo_last_keeps_the_laps_index_consistent() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");
    record_many(&mut fixture.engine, "12", 3, at(10, 1, 0), 60);

    // Act
    fixture.engine.undo_last().expect("the ride has crossings");
    let rerecord = fixture.engine.record_crossing("12", Some(at(10, 4, 0)));

    // Assert
    assert_eq!(rerecord.lap, 3);
    assert_eq!(fixture.engine.lap_times(&entry), vec![60.0, 60.0, 120.0]);
    let seqs: Vec<u32> = fixture
        .engine
        .crossings()
        .iter()
        .map(|crossing| crossing.seq)
        .collect();
    assert_eq!(seqs, vec![1, 2, 3]);
}

#[test]
fn test_undo_last_leaves_another_entries_laps_untouched() {
    // Arrange
    let mut fixture = running_fixture();
    let first = key(&fixture.roster, "12");
    let second = key(&fixture.roster, "34");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));
    let _ = fixture.engine.record_crossing("34", Some(at(10, 3, 0)));

    // Act
    fixture.engine.undo_last().expect("the ride has crossings");

    // Assert
    assert_eq!(fixture.engine.lap_times(&first), vec![120.0]);
    assert_eq!(fixture.engine.lap_times(&second), Vec::<f64>::new());
}

#[test]
fn test_undo_last_after_confirm_held_removes_the_credit() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");
    fixture
        .engine
        .confirm_held(&held.crossing)
        .expect("the card is held");
    assert_eq!(
        fixture.engine.credited_cards(&entry),
        vec![held.card.clone()]
    );

    // Act
    fixture.engine.undo_last().expect("the ride has a crossing");

    // Assert
    assert_eq!(fixture.engine.credited_cards(&entry), Vec::new());
    let redo = fixture.engine.record_crossing("12", Some(at(10, 0, 45)));
    assert_eq!(redo.card, Some(held.card));
}

#[test]
fn test_undo_last_of_a_held_card_keeps_an_earlier_credit() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let earlier = fixture.engine.record_crossing("12", Some(at(10, 20, 0)));
    let earlier_card = earlier.card.expect("the long lap deals a card");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 20, 30)));

    // Act
    fixture.engine.undo_last().expect("the ride has a crossing");

    // Assert
    assert_eq!(fixture.engine.credited_cards(&entry), vec![earlier_card]);
    assert_eq!(fixture.engine.held_crossings(), Vec::new());
    assert_eq!(fixture.engine.crossings().len(), 1);
}

// ------------------------------------------------ record a miss (K)

#[test]
fn test_pending_misses_before_any_miss_are_empty() {
    // Arrange
    let fixture = running_fixture();

    // Act
    let misses = fixture.engine.pending_misses();

    // Assert
    assert_eq!(misses, Vec::<PendingMiss>::new());
}

#[test]
fn test_record_miss_in_running_appends_the_miss_and_event() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let event = fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::RecordMiss,
            payload: json!({
                "miss_seq": 1,
                "crossed_at": "2026-09-20T10:05:00",
                "reason": "missed number",
            }),
        }
    );
    assert_eq!(
        fixture.engine.pending_misses(),
        vec![PendingMiss {
            miss_seq: 1,
            crossed_at: at(10, 5, 0),
        }]
    );
    assert_eq!(fixture.engine.events().last(), Some(&event));
}

#[test]
fn test_record_miss_in_reopened_appends_the_miss() {
    // Arrange
    let mut fixture = reopened_fixture();

    // Act
    let event = fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is reopened");

    // Assert
    assert_eq!(event.payload.get("miss_seq"), Some(&json!(1)));
    assert_eq!(fixture.engine.pending_misses().len(), 1);
}

#[test]
fn test_record_miss_numbers_misses_in_recording_order() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let first = fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");
    let second = fixture
        .engine
        .record_miss(at(10, 6, 0), "missed number")
        .expect("the ride is running");

    // Assert
    assert_eq!(
        (
            first.payload.get("miss_seq"),
            second.payload.get("miss_seq"),
        ),
        (Some(&json!(1)), Some(&json!(2)))
    );
    let seqs: Vec<u32> = fixture
        .engine
        .pending_misses()
        .iter()
        .map(|miss| miss.miss_seq)
        .collect();
    assert_eq!(seqs, vec![1, 2]);
}

#[test]
fn test_record_miss_deals_no_card_and_leaves_the_crossings_untouched() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let remaining_before = fixture.engine.shoe_remaining();

    // Act
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.crossings(), Vec::new());
    assert_eq!(fixture.engine.shoe_remaining(), remaining_before);
    assert_eq!(fixture.engine.credited_cards(&entry), Vec::new());
    assert_eq!(fixture.engine.held_crossings(), Vec::new());
}

#[test]
fn test_record_miss_is_ignored_by_undo_last() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Act
    let result = fixture.engine.undo_last();

    // Assert
    assert_eq!(result, Err(illegal("no crossings to undo")));
}

#[test]
fn test_record_miss_does_not_consume_a_shoe_card() {
    // Arrange
    let mut fixture = running_fixture();
    let expected = reference_card(fixture.engine.config(), 0);
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Act
    let result = fixture.engine.record_crossing("12", Some(at(10, 6, 0)));

    // Assert
    assert_eq!(result.card, Some(expected));
}

#[test]
fn test_record_miss_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture.engine.record_miss(at(10, 5, 0), "missed number");

    // Assert
    assert_eq!(result, Err(illegal("cannot record miss from draft")));
}

#[test]
fn test_record_miss_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let result = fixture.engine.record_miss(at(10, 5, 0), "missed number");

    // Assert
    assert_eq!(result, Err(illegal("cannot record miss from finished")));
}

#[test]
fn test_record_miss_on_a_stopped_running_ride_is_accepted() {
    // Arrange
    let mut fixture = stopped_fixture();

    // Act
    let event = fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride state is still running");

    // Assert
    assert_eq!(event.action, EventAction::RecordMiss);
    assert_eq!(fixture.engine.pending_misses().len(), 1);
}

#[test]
fn test_record_miss_with_an_empty_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.record_miss(at(10, 5, 0), "");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
    assert_eq!(fixture.engine.pending_misses(), Vec::new());
}

#[test]
fn test_record_miss_with_a_whitespace_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.record_miss(at(10, 5, 0), "   ");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
    assert_eq!(fixture.engine.pending_misses(), Vec::new());
}

// ------------------------------------------- assign_plate_to_miss (K)

#[test]
fn test_assign_plate_to_miss_removes_the_miss_and_records_the_crossing() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Act
    let event = fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified")
        .expect("the miss resolves to the roster");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::AssignPlateToMiss,
            payload: json!({
                "miss_seq": 1,
                "new_plate": "12",
                "entry_id": "key-12",
                "crossed_at": "2026-09-20T10:05:00",
                "reason": "rider identified",
            }),
        }
    );
    assert_eq!(fixture.engine.pending_misses(), Vec::new());
    assert_eq!(
        fixture.engine.crossings().to_vec(),
        vec![Crossing {
            entry_id: EntryId("key-12".to_owned()),
            seq: 1,
            crossed_at: at(10, 5, 0),
            rider_plate: Some("12".to_owned()),
            deal_seq: 0,
        }]
    );
    assert_eq!(fixture.engine.events().last(), Some(&event));
}

#[test]
fn test_assign_plate_to_miss_deals_one_card_into_the_hand() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let expected = reference_card(fixture.engine.config(), 0);
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Act
    fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified")
        .expect("the miss resolves to the roster");

    // Assert
    assert_eq!(fixture.engine.shoe.dealt(), 1);
    assert_eq!(fixture.engine.credited_cards(&entry), vec![expected]);
    assert_eq!(fixture.engine.lap_times(&entry), vec![300.0]);
}

#[test]
fn test_assign_plate_to_miss_records_the_crossing_at_the_miss_instant() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");
    fixture.clock.advance(600);

    // Act
    fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified")
        .expect("the miss resolves to the roster");

    // Assert
    assert_eq!(
        fixture.engine.crossings().first().map(|c| c.crossed_at),
        Some(at(10, 5, 0))
    );
}

#[test]
fn test_assign_plate_to_miss_with_an_unknown_plate_keeps_the_miss() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Act
    let result = fixture
        .engine
        .assign_plate_to_miss(1, "999", "rider identified");

    // Assert
    assert_eq!(
        result,
        Err(RideError::UnknownPlate {
            plate: "999".to_owned(),
        })
    );
    let seqs: Vec<u32> = fixture
        .engine
        .pending_misses()
        .iter()
        .map(|miss| miss.miss_seq)
        .collect();
    assert_eq!(seqs, vec![1]);
    assert_eq!(fixture.engine.crossings(), Vec::new());
}

#[test]
fn test_assign_plate_to_miss_with_miss_seq_zero_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Act
    let result = fixture
        .engine
        .assign_plate_to_miss(0, "12", "rider identified");

    // Assert
    assert_eq!(result, Err(illegal("no pending miss with miss_seq 0")));
}

#[test]
fn test_assign_plate_to_miss_with_miss_seq_two_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Act
    let result = fixture
        .engine
        .assign_plate_to_miss(2, "12", "rider identified");

    // Assert
    assert_eq!(result, Err(illegal("no pending miss with miss_seq 2")));
}

#[test]
fn test_assign_plate_to_miss_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified");

    // Assert
    assert_eq!(
        result,
        Err(illegal("cannot assign plate to miss from draft"))
    );
}

#[test]
fn test_assign_plate_to_miss_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let result = fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified");

    // Assert
    assert_eq!(
        result,
        Err(illegal("cannot assign plate to miss from finished"))
    );
}

#[test]
fn test_assign_plate_to_miss_in_reopened_records_the_crossing() {
    // Arrange
    let mut fixture = reopened_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is reopened");

    // Act
    fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified")
        .expect("the miss resolves to the roster");

    // Assert
    assert_eq!(fixture.engine.crossings().len(), 1);
    assert_eq!(fixture.engine.pending_misses(), Vec::new());
}

#[test]
fn test_assign_plate_to_miss_with_an_empty_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Act
    let result = fixture.engine.assign_plate_to_miss(1, "12", "");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
    assert_eq!(fixture.engine.pending_misses().len(), 1);
}

#[test]
fn test_assign_plate_to_miss_with_a_whitespace_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Act
    let result = fixture.engine.assign_plate_to_miss(1, "12", "   ");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
    assert_eq!(fixture.engine.pending_misses().len(), 1);
}

#[test]
fn test_assign_plate_to_miss_never_holds_a_short_crossing() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 60,
        hold_short_laps: true,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");
    fixture
        .engine
        .record_miss(at(10, 0, 30), "missed number")
        .expect("the ride is running");

    // Act
    fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified")
        .expect("the miss resolves to the roster");

    // Assert
    assert_eq!(fixture.engine.held_crossings(), Vec::new());
    assert_eq!(fixture.engine.credited_cards(&entry).len(), 1);
    assert_eq!(fixture.engine.lap_times(&entry), vec![30.0]);
}

#[test]
fn test_assign_plate_to_miss_removes_only_the_named_miss() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");
    fixture
        .engine
        .record_miss(at(10, 6, 0), "missed number")
        .expect("the ride is running");

    // Act
    fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified")
        .expect("the miss resolves to the roster");

    // Assert
    assert_eq!(
        fixture.engine.pending_misses(),
        vec![PendingMiss {
            miss_seq: 2,
            crossed_at: at(10, 6, 0),
        }]
    );
}

#[test]
fn test_assign_plate_to_miss_numbers_the_next_lap_after_existing_crossings() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 2, 0)));
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Act
    fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified")
        .expect("the miss resolves to the roster");

    // Assert
    assert_eq!(fixture.engine.crossings()[1].seq, 2);
    assert_eq!(fixture.engine.lap_times(&entry), vec![120.0, 180.0]);
}

#[test]
fn test_assign_plate_to_miss_keeps_the_miss_counter_after_a_resolution() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");
    fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified")
        .expect("the miss resolves to the roster");

    // Act
    let second = fixture
        .engine
        .record_miss(at(10, 6, 0), "missed number")
        .expect("the ride is running");

    // Assert
    assert_eq!(second.payload.get("miss_seq"), Some(&json!(2)));
}

#[test]
fn test_assign_plate_to_miss_marks_the_entry_has_data() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");

    // Act
    fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified")
        .expect("the miss resolves to the roster");

    // Assert
    assert_eq!(fixture.roster.has_data(), vec![entry]);
}

#[test]
fn test_assign_plate_to_miss_with_a_closed_shoe_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");
    fixture.engine.shoe.close();

    // Act
    let result = fixture
        .engine
        .assign_plate_to_miss(1, "12", "rider identified");

    // Assert
    assert_eq!(result, Err(illegal("shoe is closed")));
    assert_eq!(fixture.engine.crossings(), Vec::new());
}

// -------------------------------------- held-card lifecycle (R-34)

#[test]
fn test_confirm_held_credits_the_card_and_appends_the_event() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");

    // Act
    let event = fixture
        .engine
        .confirm_held(&held.crossing)
        .expect("the card is held");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::ConfirmHeld,
            payload: json!({
                "entry_id": "key-12",
                "seq": 1,
                "card": held.card.code(),
            }),
        }
    );
    assert_eq!(fixture.engine.held_crossings(), Vec::new());
    assert_eq!(
        fixture.engine.credited_cards(&entry),
        vec![held.card.clone()]
    );
    assert_eq!(fixture.engine.held_card_for(&held.crossing), None);
    assert!(!fixture.engine.is_card_voided(&held.crossing));
}

#[test]
fn test_confirm_held_on_a_released_crossing_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");
    fixture
        .engine
        .confirm_held(&held.crossing)
        .expect("the card is held");

    // Act
    let result = fixture.engine.confirm_held(&held.crossing);

    // Assert
    assert_eq!(result, Err(illegal("crossing's card is not held")));
}

#[test]
fn test_confirm_held_on_a_credited_crossing_is_refused() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let crossing = fixture
        .engine
        .crossings()
        .first()
        .cloned()
        .expect("the lap is recorded");

    // Act
    let result = fixture.engine.confirm_held(&crossing);

    // Assert
    assert_eq!(result, Err(illegal("crossing's card is not held")));
}

#[test]
fn test_void_held_marks_the_card_voided_and_never_credits() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");

    // Act
    let event = fixture
        .engine
        .void_held(&held.crossing)
        .expect("the card is held");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::VoidHeld,
            payload: json!({
                "entry_id": "key-12",
                "seq": 1,
                "card": held.card.code(),
            }),
        }
    );
    assert_eq!(fixture.engine.held_crossings(), Vec::new());
    assert_eq!(fixture.engine.credited_cards(&entry), Vec::new());
    assert!(fixture.engine.is_card_voided(&held.crossing));
}

#[test]
fn test_void_held_twice_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");
    fixture
        .engine
        .void_held(&held.crossing)
        .expect("the card is held");

    // Act
    let result = fixture.engine.void_held(&held.crossing);

    // Assert
    assert_eq!(result, Err(illegal("crossing's card is not held")));
}

#[test]
fn test_void_held_on_a_credited_crossing_is_refused() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let crossing = fixture
        .engine
        .crossings()
        .first()
        .cloned()
        .expect("the lap is recorded");

    // Act
    let result = fixture.engine.void_held(&crossing);

    // Assert
    assert_eq!(result, Err(illegal("crossing's card is not held")));
}

#[test]
fn test_return_to_held_moves_a_credited_card_into_the_hold() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");
    let recorded = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let card = recorded.card.expect("the lap deals a card");
    let crossing = fixture
        .engine
        .crossings()
        .first()
        .cloned()
        .expect("the lap is recorded");

    // Act
    let event = fixture
        .engine
        .return_to_held(&crossing)
        .expect("the crossing was dealt");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::ReturnToHeld,
            payload: json!({
                "entry_id": "key-12",
                "seq": 1,
                "card": card.code(),
            }),
        }
    );
    assert_eq!(fixture.engine.credited_cards(&entry), Vec::new());
    assert_eq!(fixture.engine.held_card_for(&crossing), Some(card.clone()));
    assert_eq!(
        fixture.engine.held_crossings(),
        vec![HeldCrossing {
            crossing: crossing.clone(),
            card,
        }]
    );
    assert_eq!(fixture.engine.shoe.dealt(), 1);
}

#[test]
fn test_return_to_held_on_an_already_held_crossing_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let crossing = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card")
        .crossing;

    // Act
    let result = fixture.engine.return_to_held(&crossing);

    // Assert
    assert_eq!(result, Err(illegal("crossing's card is already held")));
}

#[test]
fn test_return_to_held_of_a_voided_card_deals_a_fresh_card() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let crossing = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card")
        .crossing;
    let old = fixture
        .engine
        .card_for(&crossing)
        .expect("the crossing was dealt");
    fixture
        .engine
        .void_held(&crossing)
        .expect("the card is held");
    let expected = reference_card(fixture.engine.config(), 1);
    let dealt_before = fixture.engine.shoe.dealt();

    // Act
    let event = fixture
        .engine
        .return_to_held(&crossing)
        .expect("the crossing was dealt");

    // Assert
    let fresh = fixture
        .engine
        .card_for(&crossing)
        .expect("the fresh deal is recorded");
    assert_eq!(fresh, expected);
    assert_ne!(fresh, old);
    assert_eq!(fixture.engine.shoe.dealt(), dealt_before + 1);
    assert_eq!(fixture.engine.held_card_for(&crossing), Some(fresh.clone()));
    assert_eq!(event.payload.get("card"), Some(&json!(fresh.code())));
    assert!(!fixture.engine.is_card_voided(&crossing));
}

#[test]
fn test_return_to_held_of_a_voided_card_in_reopened_deals_a_fresh_card() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let crossing = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card")
        .crossing;
    let old = fixture
        .engine
        .card_for(&crossing)
        .expect("the crossing was dealt");
    fixture
        .engine
        .void_held(&crossing)
        .expect("the card is held");
    fixture.engine.finish(&[]).expect("the ride is running");
    fixture.engine.reopen().expect("the ride is finished");

    // Act
    fixture
        .engine
        .return_to_held(&crossing)
        .expect("the reopened shoe deals");

    // Assert
    let fresh = fixture
        .engine
        .card_for(&crossing)
        .expect("the fresh deal is recorded");
    assert_ne!(fresh, old);
    assert_eq!(fixture.engine.held_card_for(&crossing), Some(fresh));
}

#[test]
fn test_return_to_held_of_a_voided_card_after_finish_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let crossing = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card")
        .crossing;
    fixture
        .engine
        .void_held(&crossing)
        .expect("the card is held");
    fixture.engine.finish(&[]).expect("the ride is running");

    // Act
    let result = fixture.engine.return_to_held(&crossing);

    // Assert
    assert_eq!(
        result,
        Err(illegal("cannot re-deal a voided card from finished"))
    );
}

#[test]
fn test_return_to_held_of_a_voided_card_with_a_closed_shoe_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let crossing = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card")
        .crossing;
    fixture
        .engine
        .void_held(&crossing)
        .expect("the card is held");
    fixture.engine.shoe.close();

    // Act
    let result = fixture.engine.return_to_held(&crossing);

    // Assert
    assert_eq!(result, Err(illegal("shoe is closed")));
}

#[test]
fn test_return_to_held_after_finish_re_holds_a_credited_card() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let recorded = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let card = recorded.card.expect("the lap deals a card");
    let crossing = fixture
        .engine
        .crossings()
        .first()
        .cloned()
        .expect("the lap is recorded");
    fixture.engine.finish(&[]).expect("the ride is running");

    // Act
    fixture
        .engine
        .return_to_held(&crossing)
        .expect("a credited card re-holds in any state");

    // Assert
    assert_eq!(fixture.engine.state(), RideStatus::Finished);
    assert_eq!(fixture.engine.held_card_for(&crossing), Some(card));
}

#[test]
fn test_return_to_held_on_a_never_dealt_crossing_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let crossing = Crossing {
        entry_id: EntryId("key-12".to_owned()),
        seq: 99,
        crossed_at: at(10, 30, 0),
        rider_plate: Some("12".to_owned()),
        deal_seq: 99,
    };

    // Act
    let result = fixture.engine.return_to_held(&crossing);

    // Assert
    assert_eq!(
        result,
        Err(RideError::NoCrossing {
            entry_id: "key-12".to_owned(),
            seq: 99,
        })
    );
}

#[test]
fn test_return_to_held_then_confirm_credits_the_card() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");
    let recorded = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let card = recorded.card.expect("the lap deals a card");
    let crossing = fixture
        .engine
        .crossings()
        .first()
        .cloned()
        .expect("the lap is recorded");
    fixture
        .engine
        .return_to_held(&crossing)
        .expect("the crossing was dealt");

    // Act
    fixture
        .engine
        .confirm_held(&crossing)
        .expect("the card is held");

    // Assert
    assert_eq!(fixture.engine.credited_cards(&entry), vec![card.clone()]);
    assert_eq!(fixture.engine.held_card_for(&crossing), None);
}

#[test]
fn test_held_card_for_a_credited_crossing_is_none() {
    // Arrange
    let config = config_with(RideConfigParams {
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture(config, FakeRoster::new(two_entries()));
    fixture.engine.start(None).expect("the roster is ready");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let crossing = fixture
        .engine
        .crossings()
        .first()
        .cloned()
        .expect("the lap is recorded");

    // Act
    let card = fixture.engine.held_card_for(&crossing);

    // Assert
    assert_eq!(card, None);
}

#[test]
fn test_held_card_for_a_held_crossing_returns_its_card() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");

    // Act
    let card = fixture.engine.held_card_for(&held.crossing);

    // Assert
    assert_eq!(card, Some(held.card));
}

#[test]
fn test_card_for_a_held_crossing_returns_the_dealt_card() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");

    // Act
    let card = fixture.engine.card_for(&held.crossing);

    // Assert
    assert_eq!(card, Some(held.card));
}

#[test]
fn test_card_for_a_never_dealt_crossing_is_none() {
    // Arrange
    let fixture = running_fixture();
    let crossing = Crossing {
        entry_id: EntryId("key-12".to_owned()),
        seq: 99,
        crossed_at: at(10, 30, 0),
        rider_plate: Some("12".to_owned()),
        deal_seq: 99,
    };

    // Act
    let card = fixture.engine.card_for(&crossing);

    // Assert
    assert_eq!(card, None);
}

// ---------------------------------------- same-code identity (E7.1.1)

#[test]
fn test_identity_fixture_deals_one_code_on_two_deal_slots() {
    // Arrange
    let mut fixture = identity_fixture(1);
    fixture.engine.start(None).expect("the roster is ready");

    // Act
    let (first, second) = record_identity_pair(&mut fixture, at(10, 0, 20));
    let first_card = fixture
        .engine
        .card_for(&first)
        .expect("the sixth deal is recorded");
    let second_card = fixture
        .engine
        .card_for(&second)
        .expect("the seventh deal is recorded");

    // Assert
    assert_eq!(first_card, second_card);
    assert_eq!((first.deal_seq, second.deal_seq), (5, 6));
}

#[test]
fn test_identity_two_same_code_held_cards_are_voided_independently() {
    // Arrange
    let mut fixture = identity_fixture(60);
    fixture.engine.start(None).expect("the roster is ready");
    let (first, second) = record_identity_pair(&mut fixture, at(10, 0, 14));
    let second_card = fixture
        .engine
        .card_for(&second)
        .expect("the seventh deal is recorded");

    // Act
    fixture
        .engine
        .void_held(&first)
        .expect("the sixth card is held");

    // Assert
    assert!(fixture.engine.is_card_voided(&first));
    assert!(!fixture.engine.is_card_voided(&second));
    assert_eq!(fixture.engine.held_card_for(&first), None);
    assert_eq!(fixture.engine.held_card_for(&second), Some(second_card));
    assert_eq!(fixture.engine.held_crossings().len(), 6);
}

#[test]
fn test_identity_confirm_of_one_same_code_held_card_leaves_the_sibling_held() {
    // Arrange
    let mut fixture = identity_fixture(60);
    fixture.engine.start(None).expect("the roster is ready");
    let (first, second) = record_identity_pair(&mut fixture, at(10, 0, 14));
    let first_card = fixture
        .engine
        .card_for(&first)
        .expect("the sixth deal is recorded");
    let second_card = fixture
        .engine
        .card_for(&second)
        .expect("the seventh deal is recorded");
    let entry = key(&fixture.roster, "12");

    // Act
    fixture
        .engine
        .confirm_held(&first)
        .expect("the sixth card is held");

    // Assert
    assert_eq!(fixture.engine.credited_cards(&entry), vec![first_card]);
    assert_eq!(fixture.engine.held_card_for(&second), Some(second_card));
    assert_eq!(fixture.engine.held_crossings().len(), 6);
}

#[test]
fn test_identity_void_of_one_same_code_card_leaves_the_sibling_confirmable() {
    // Arrange
    let mut fixture = identity_fixture(60);
    fixture.engine.start(None).expect("the roster is ready");
    let (first, second) = record_identity_pair(&mut fixture, at(10, 0, 14));
    let second_card = fixture
        .engine
        .card_for(&second)
        .expect("the seventh deal is recorded");
    let entry = key(&fixture.roster, "12");
    fixture
        .engine
        .void_held(&first)
        .expect("the sixth card is held");

    // Act
    fixture
        .engine
        .confirm_held(&second)
        .expect("the seventh card is still held");

    // Assert
    assert_eq!(fixture.engine.credited_cards(&entry), vec![second_card]);
    assert!(!fixture.engine.is_card_voided(&second));
}

#[test]
fn test_identity_discard_credited_matches_the_first_value_occurrence() {
    // Arrange
    let mut fixture = identity_fixture(1);
    fixture.engine.start(None).expect("the roster is ready");
    let entry = key(&fixture.roster, "12");
    let (first, second) = record_identity_pair(&mut fixture, at(10, 0, 20));
    let first_card = fixture
        .engine
        .card_for(&first)
        .expect("the sixth deal is recorded");
    let second_card = fixture
        .engine
        .card_for(&second)
        .expect("the seventh deal is recorded");
    assert_eq!(first_card, second_card);

    // Act
    fixture
        .engine
        .return_to_held(&second)
        .expect("the crossing was dealt");

    // Assert
    let hand = fixture
        .engine
        .hand
        .get(&entry)
        .expect("the entry has a credited hand");
    let deal_seqs: Vec<u64> = hand.iter().map(|(deal_seq, _)| *deal_seq).collect();
    assert_eq!(deal_seqs, vec![0, 1, 2, 3, 4, 6]);
    assert_eq!(fixture.engine.held_card_for(&second), Some(second_card));
}

// ============================================ E7.1.1 audited corrections
// ================================================ and E3.1.2 rider moves

fn min_lap_config(min_lap_s: i32) -> RideConfig {
    config_with(RideConfigParams {
        min_lap_s,
        ..params()
    })
}

fn started_with(config: RideConfig, roster: FakeRoster) -> Fixture {
    let mut fixture = fixture(config, roster);
    fixture.engine.start(None).expect("the roster is ready");
    fixture
}

fn stopped_with(config: RideConfig, roster: FakeRoster) -> Fixture {
    let mut fixture = started_with(config, roster);
    fixture.engine.stop().expect("the ride is running");
    fixture
}

fn running_entries(entries: Vec<EntrySnapshot>) -> Fixture {
    started_with(fixture_config(), FakeRoster::new(entries))
}

fn pooled_stopped(entries: Vec<EntrySnapshot>) -> Fixture {
    stopped_with(fixture_config(), FakeRoster::new(entries))
}

fn pooled_running_with(config: RideConfig, entries: Vec<EntrySnapshot>) -> Fixture {
    started_with(config, FakeRoster::new(entries))
}

fn laps_of(fixture: &Fixture, entry_id: &EntryId) -> Vec<Crossing> {
    fixture
        .engine
        .laps
        .get(entry_id)
        .cloned()
        .unwrap_or_default()
}

fn last_crossing(fixture: &Fixture) -> Crossing {
    fixture
        .engine
        .crossings()
        .last()
        .cloned()
        .expect("a crossing is recorded")
}

fn dealt_card(fixture: &Fixture, crossing: &Crossing) -> Card {
    fixture
        .engine
        .card_for(crossing)
        .expect("the crossing was dealt a card")
}

fn seqs(laps: &[Crossing]) -> Vec<u32> {
    laps.iter().map(|crossing| crossing.seq).collect()
}

fn rider_plates(laps: &[Crossing]) -> Vec<Option<String>> {
    laps.iter()
        .map(|crossing| crossing.rider_plate.clone())
        .collect()
}

fn record_at(fixture: &mut Fixture, plate: &str, instant: Timestamp) -> CrossingResult {
    fixture.engine.record_crossing(plate, Some(instant))
}

// ------------------------------------------------------ deal manual (spec §4)

#[test]
fn test_deal_manual_credits_the_next_shoe_card_and_audits_the_reason() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let expected = reference_card(fixture.engine.config(), 0);

    // Act
    let event = fixture
        .engine
        .deal_manual("12", Some("bonus card"))
        .expect("the ride is running");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::DealManual,
            payload: json!({
                "plate": "12",
                "entry_id": "key-12",
                "card": expected.code(),
                "reason": "bonus card",
            }),
        }
    );
    assert_eq!(fixture.engine.credited_cards(&entry), vec![expected]);
}

#[test]
fn test_deal_manual_without_a_reason_records_an_empty_reason() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let event = fixture
        .engine
        .deal_manual("12", None)
        .expect("the ride is running");

    // Assert
    assert_eq!(event.payload.get("reason"), Some(&json!("")));
    assert_eq!(event.action, EventAction::DealManual);
}

#[test]
fn test_deal_manual_marks_the_entry_has_data() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    fixture
        .engine
        .deal_manual("12", Some("bonus card"))
        .expect("the ride is running");

    // Assert
    assert_eq!(fixture.roster.has_data(), vec![entry]);
}

#[test]
fn test_deal_manual_for_a_pooled_rider_credits_the_team() {
    // Arrange
    let mut fixture = running_entries(vec![pooled_team()]);
    let team = key(&fixture.roster, "9");

    // Act
    let event = fixture
        .engine
        .deal_manual("45", Some("bonus card"))
        .expect("the ride is running");

    // Assert
    assert_eq!(event.payload.get("entry_id"), Some(&json!("key-team")));
    assert_eq!(fixture.engine.credited_cards(&team).len(), 1);
}

#[test]
fn test_deal_manual_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture.engine.deal_manual("12", Some("bonus card"));

    // Assert
    assert_eq!(result, Err(illegal("cannot deal manually from draft")));
}

#[test]
fn test_deal_manual_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let result = fixture.engine.deal_manual("12", Some("bonus card"));

    // Assert
    assert_eq!(result, Err(illegal("cannot deal manually from finished")));
}

#[test]
fn test_deal_manual_with_an_unknown_plate_is_refused_without_dealing() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.deal_manual("999", Some("bonus card"));

    // Assert
    assert_eq!(
        result,
        Err(RideError::UnknownPlate {
            plate: "999".to_owned(),
        })
    );
    assert_eq!(fixture.engine.shoe_remaining(), fixture.engine.shoe_total());
}

#[test]
fn test_deal_manual_in_reopened_deals_from_the_open_shoe() {
    // Arrange
    let mut fixture = reopened_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let event = fixture
        .engine
        .deal_manual("12", Some("bonus card"))
        .expect("the reopened shoe deals");

    // Assert
    assert_eq!(event.action, EventAction::DealManual);
    assert_eq!(fixture.engine.credited_cards(&entry).len(), 1);
}

#[test]
fn test_deal_manual_with_a_closed_shoe_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.engine.shoe.close();
    let message = fixture
        .engine
        .deal_card()
        .expect_err("the closed shoe refuses")
        .to_string();

    // Act
    let result = fixture.engine.deal_manual("12", Some("bonus card"));

    // Assert
    assert_eq!(result, Err(illegal(&message)));
    assert_eq!(
        fixture.engine.credited_cards(&key(&fixture.roster, "12")),
        vec![]
    );
}

// -------------------------------------------------------- edit crossing

#[test]
fn test_edit_crossing_audits_both_times_and_the_reason() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let before = fixture.engine.events().len();

    // Act
    let event = fixture
        .engine
        .edit_crossing(&entry, 1, at(10, 31, 0), "mis-keyed time")
        .expect("the crossing is live");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::EditCrossing,
            payload: json!({
                "entry_id": "key-12",
                "seq": 1,
                "previous_crossed_at": "2026-09-20T10:30:00",
                "crossed_at": "2026-09-20T10:31:00",
                "reason": "mis-keyed time",
            }),
        }
    );
    assert_eq!(fixture.engine.events().len(), before + 1);
}

#[test]
fn test_edit_crossing_moves_only_the_timestamp_and_recomputes_laps() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let _ = record_at(&mut fixture, "12", at(10, 40, 0));
    let entry = key(&fixture.roster, "12");

    // Act
    fixture
        .engine
        .edit_crossing(&entry, 2, at(10, 35, 0), "mis-keyed time")
        .expect("the crossing is live");

    // Assert
    assert_eq!(fixture.engine.lap_times(&entry), vec![1800.0, 300.0]);
}

#[test]
fn test_edit_crossing_keeps_the_card_and_the_shoe_untouched() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let second = record_at(&mut fixture, "12", at(10, 40, 0));
    let entry = key(&fixture.roster, "12");
    let dealt_before = fixture.engine.shoe.dealt();

    // Act
    fixture
        .engine
        .edit_crossing(&entry, 2, at(10, 35, 0), "mis-keyed time")
        .expect("the crossing is live");

    // Assert
    assert_eq!(fixture.engine.shoe.dealt(), dealt_before);
    assert_eq!(
        fixture.engine.card_for(&last_crossing(&fixture)),
        second.card
    );
    assert_eq!(last_crossing(&fixture).crossed_at, at(10, 35, 0));
}

#[test]
fn test_edit_crossing_keeps_a_held_card_held() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 0, 30));
    let held_before = fixture.engine.held_crossings()[0].clone();

    // Act
    fixture
        .engine
        .edit_crossing(&entry, 1, at(10, 0, 45), "mis-keyed time")
        .expect("the crossing is live");

    // Assert
    let held = fixture.engine.held_crossings();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].crossing.seq, 1);
    assert_eq!(held[0].crossing.crossed_at, at(10, 0, 45));
    assert_eq!(held[0].card, held_before.card);
}

#[test]
fn test_edit_crossing_with_an_empty_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture.engine.edit_crossing(&entry, 1, at(10, 31, 0), "");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_edit_crossing_with_a_whitespace_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture
        .engine
        .edit_crossing(&entry, 1, at(10, 31, 0), " \t\n");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_edit_crossing_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let result = fixture
        .engine
        .edit_crossing(&entry, 1, at(10, 31, 0), "mis-keyed time");

    // Assert
    assert_eq!(result, Err(illegal("cannot edit crossing from draft")));
}

#[test]
fn test_edit_crossing_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let result = fixture
        .engine
        .edit_crossing(&entry, 1, at(10, 31, 0), "mis-keyed time");

    // Assert
    assert_eq!(result, Err(illegal("cannot edit crossing from finished")));
}

#[test]
fn test_edit_crossing_with_seq_zero_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture
        .engine
        .edit_crossing(&entry, 0, at(10, 31, 0), "mis-keyed time");

    // Assert
    assert_eq!(
        result,
        Err(RideError::NoCrossing {
            entry_id: "key-12".to_owned(),
            seq: 0,
        })
    );
}

#[test]
fn test_edit_crossing_with_an_unknown_seq_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture
        .engine
        .edit_crossing(&entry, 9, at(10, 31, 0), "mis-keyed time");

    // Assert
    assert_eq!(
        result,
        Err(RideError::NoCrossing {
            entry_id: "key-12".to_owned(),
            seq: 9,
        })
    );
}

#[test]
fn test_edit_crossing_of_lap_one_at_the_start_instant_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture
        .engine
        .edit_crossing(&entry, 1, at(10, 0, 0), "mis-keyed time");

    // Assert
    assert_eq!(
        result,
        Err(RideError::LapTimeNotPositive {
            entry_label: "12".to_owned(),
            crossed_at: "2026-09-20T10:00:00".to_owned(),
            previous: "2026-09-20T10:00:00".to_owned(),
        })
    );
}

#[test]
fn test_edit_crossing_of_a_later_lap_at_its_predecessor_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let _ = record_at(&mut fixture, "12", at(10, 40, 0));

    // Act
    let result = fixture
        .engine
        .edit_crossing(&entry, 2, at(10, 30, 0), "mis-keyed time");

    // Assert
    assert_eq!(
        result,
        Err(RideError::LapTimeNotPositive {
            entry_label: "12".to_owned(),
            crossed_at: "2026-09-20T10:30:00".to_owned(),
            previous: "2026-09-20T10:30:00".to_owned(),
        })
    );
}

#[test]
fn test_edit_crossing_without_a_start_instant_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    fixture.engine.actual_start = None;

    // Act
    let result = fixture
        .engine
        .edit_crossing(&entry, 1, at(10, 31, 0), "mis-keyed time");

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
}

// -------------------------------------------------------- void crossing

#[test]
fn test_void_crossing_audits_the_entry_seq_and_reason() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let before = fixture.engine.events().len();

    // Act
    let event = fixture
        .engine
        .void_crossing(&entry, 1, "double entry")
        .expect("the crossing is live");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::VoidCrossing,
            payload: json!({
                "entry_id": "key-12",
                "seq": 1,
                "reason": "double entry",
            }),
        }
    );
    assert_eq!(fixture.engine.events().len(), before + 1);
}

#[test]
fn test_void_crossing_removes_the_lap_and_renumbers_later_laps() {
    // Arrange
    let config = min_lap_config(1);
    let mut fixture = started_with(config, FakeRoster::new(two_entries()));
    let entry = key(&fixture.roster, "12");
    let first = record_at(&mut fixture, "12", at(10, 30, 0));
    let second = record_at(&mut fixture, "12", at(10, 32, 0));
    let third = record_at(&mut fixture, "12", at(10, 34, 0));

    // Act
    fixture
        .engine
        .void_crossing(&entry, 2, "double entry")
        .expect("the crossing is live");

    // Assert
    assert_eq!(seqs(fixture.engine.crossings()), vec![1, 2]);
    assert_eq!(
        fixture
            .engine
            .crossings()
            .iter()
            .map(|crossing| crossing.crossed_at)
            .collect::<Vec<_>>(),
        vec![at(10, 30, 0), at(10, 34, 0)]
    );
    assert_eq!(fixture.engine.lap_times(&entry), vec![1800.0, 240.0]);
    let credited: Vec<Card> = vec![
        first.card.expect("the first lap deals a card"),
        third.card.expect("the third lap deals a card"),
    ];
    assert_eq!(fixture.engine.credited_cards(&entry), credited);
    assert!(
        !fixture
            .engine
            .credited_cards(&entry)
            .contains(&second.card.expect("the second lap deals a card"))
    );
}

#[test]
fn test_void_crossing_marks_the_card_voided_and_drops_it_from_the_hand() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let crossing = last_crossing(&fixture);

    // Act
    fixture
        .engine
        .void_crossing(&entry, 1, "double entry")
        .expect("the crossing is live");

    // Assert
    assert_eq!(fixture.engine.credited_cards(&entry), vec![]);
    assert_eq!(fixture.engine.is_card_voided(&crossing), true);
    assert_eq!(fixture.engine.card_for(&crossing), None);
}

#[test]
fn test_void_crossing_does_not_restitute_the_card() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let dealt_before = fixture.engine.shoe.dealt();

    // Act
    fixture
        .engine
        .void_crossing(&entry, 1, "double entry")
        .expect("the crossing is live");

    // Assert
    assert_eq!(fixture.engine.shoe.dealt(), dealt_before);
}

#[test]
fn test_void_crossing_of_a_held_crossing_releases_the_hold() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 0, 30));
    assert_eq!(fixture.engine.held_crossings().len(), 1);

    // Act
    fixture
        .engine
        .void_crossing(&entry, 1, "double entry")
        .expect("the crossing is live");

    // Assert
    assert_eq!(fixture.engine.held_crossings(), vec![]);
    assert_eq!(fixture.engine.crossings(), Vec::new());
}

#[test]
fn test_void_crossing_with_an_empty_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture.engine.void_crossing(&entry, 1, "");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_void_crossing_with_a_whitespace_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture.engine.void_crossing(&entry, 1, "  ");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_void_crossing_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let result = fixture.engine.void_crossing(&entry, 1, "double entry");

    // Assert
    assert_eq!(result, Err(illegal("cannot void crossing from draft")));
}

#[test]
fn test_void_crossing_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();
    let entry = key(&fixture.roster, "12");

    // Act
    let result = fixture.engine.void_crossing(&entry, 1, "double entry");

    // Assert
    assert_eq!(result, Err(illegal("cannot void crossing from finished")));
}

#[test]
fn test_void_crossing_with_seq_zero_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture.engine.void_crossing(&entry, 0, "double entry");

    // Assert
    assert_eq!(
        result,
        Err(RideError::NoCrossing {
            entry_id: "key-12".to_owned(),
            seq: 0,
        })
    );
}

#[test]
fn test_void_crossing_with_an_unknown_seq_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture.engine.void_crossing(&entry, 9, "double entry");

    // Assert
    assert_eq!(
        result,
        Err(RideError::NoCrossing {
            entry_id: "key-12".to_owned(),
            seq: 9,
        })
    );
}

#[test]
fn test_void_crossing_without_a_dealt_card_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let crossing = last_crossing(&fixture);
    fixture.engine.dealt.remove(&crossing.deal_seq);

    // Act
    let result = fixture.engine.void_crossing(&entry, 1, "double entry");

    // Assert
    assert_eq!(result, Err(illegal("crossing has no dealt card")));
}

// ------------------------------------------------- add crossing at time

#[test]
fn test_add_crossing_at_audits_the_plate_entry_time_and_reason() {
    // Arrange
    let mut fixture = running_fixture();
    let before = fixture.engine.events().len();

    // Act
    let event = fixture
        .engine
        .add_crossing_at("12", at(10, 15, 0), "missed crossing")
        .expect("the ride is running");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::AddCrossingAt,
            payload: json!({
                "plate": "12",
                "entry_id": "key-12",
                "crossed_at": "2026-09-20T10:15:00",
                "reason": "missed crossing",
            }),
        }
    );
    assert_eq!(fixture.engine.events().len(), before + 1);
}

#[test]
fn test_add_crossing_at_deals_the_next_card_and_credits_the_hand() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let expected = reference_card(fixture.engine.config(), 0);

    // Act
    fixture
        .engine
        .add_crossing_at("12", at(10, 15, 0), "missed crossing")
        .expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.shoe.dealt(), 1);
    assert_eq!(fixture.engine.lap_times(&entry), vec![900.0]);
    assert_eq!(fixture.engine.credited_cards(&entry), vec![expected]);
    assert_eq!(fixture.roster.has_data(), vec![entry]);
}

#[test]
fn test_add_crossing_at_never_holds_a_short_lap() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    fixture
        .engine
        .add_crossing_at("12", at(10, 0, 30), "missed crossing")
        .expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.held_crossings(), vec![]);
    assert_eq!(
        fixture
            .engine
            .credited_cards(&key(&fixture.roster, "12"))
            .len(),
        1
    );
}

#[test]
fn test_add_crossing_at_appends_after_the_entrys_laps() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    fixture
        .engine
        .add_crossing_at("12", at(10, 50, 0), "missed crossing")
        .expect("the ride is running");

    // Assert
    assert_eq!(seqs(fixture.engine.crossings()), vec![1, 2]);
    assert_eq!(fixture.engine.lap_times(&entry), vec![1800.0, 1200.0]);
}

#[test]
fn test_add_crossing_at_with_an_empty_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.add_crossing_at("12", at(10, 15, 0), "");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_add_crossing_at_with_a_whitespace_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.add_crossing_at("12", at(10, 15, 0), "\t ");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_add_crossing_at_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture
        .engine
        .add_crossing_at("12", at(10, 15, 0), "missed crossing");

    // Assert
    assert_eq!(result, Err(illegal("cannot add crossing from draft")));
}

#[test]
fn test_add_crossing_at_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let result = fixture
        .engine
        .add_crossing_at("12", at(10, 15, 0), "missed crossing");

    // Assert
    assert_eq!(result, Err(illegal("cannot add crossing from finished")));
}

#[test]
fn test_add_crossing_at_with_an_unknown_plate_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture
        .engine
        .add_crossing_at("999", at(10, 15, 0), "missed crossing");

    // Assert
    assert_eq!(
        result,
        Err(RideError::UnknownPlate {
            plate: "999".to_owned(),
        })
    );
}

#[test]
fn test_add_crossing_at_in_reopened_deals_from_the_open_shoe() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    fixture.engine.finish(&[]).expect("the ride is running");
    fixture.engine.reopen().expect("the ride is finished");

    // Act
    let event = fixture
        .engine
        .add_crossing_at("12", at(10, 35, 0), "missed crossing")
        .expect("the reopened shoe deals");

    // Assert
    assert_eq!(event.action, EventAction::AddCrossingAt);
    assert_eq!(fixture.engine.shoe.dealt(), 2);
    assert_eq!(fixture.engine.lap_times(&entry).len(), 2);
}

#[test]
fn test_add_crossing_at_at_the_latest_instant_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture
        .engine
        .add_crossing_at("12", at(10, 30, 0), "missed crossing");

    // Assert
    assert_eq!(
        result,
        Err(RideError::LapTimeNotPositive {
            entry_label: "12".to_owned(),
            crossed_at: "2026-09-20T10:30:00".to_owned(),
            previous: "2026-09-20T10:30:00".to_owned(),
        })
    );
}

#[test]
fn test_add_crossing_at_before_the_start_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture
        .engine
        .add_crossing_at("12", at(9, 59, 0), "missed crossing");

    // Assert
    assert_eq!(
        result,
        Err(RideError::LapTimeNotPositive {
            entry_label: "12".to_owned(),
            crossed_at: "2026-09-20T09:59:00".to_owned(),
            previous: "2026-09-20T10:00:00".to_owned(),
        })
    );
}

#[test]
fn test_add_crossing_at_without_a_start_instant_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.engine.actual_start = None;

    // Act
    let result = fixture
        .engine
        .add_crossing_at("12", at(10, 15, 0), "missed crossing");

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
}

// ----------------------------------------------------- reassign crossing

#[test]
fn test_reassign_crossing_audits_the_seq_and_both_entries() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let _ = record_at(&mut fixture, "12", at(10, 32, 0));
    let _ = record_at(&mut fixture, "34", at(10, 34, 0));
    let before = fixture.engine.events().len();

    // Act
    let event = fixture
        .engine
        .reassign_crossing(2, "34", "mis-keyed plate")
        .expect("the ordinal names a crossing");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::Reassign,
            payload: json!({
                "seq": 2,
                "old_entry_id": "key-12",
                "new_entry_id": "key-34",
                "new_plate": "34",
                "reason": "mis-keyed plate",
            }),
        }
    );
    assert_eq!(fixture.engine.events().len(), before + 1);
}

#[test]
fn test_reassign_crossing_moves_the_crossing_and_its_card() {
    // Arrange
    let config = min_lap_config(1);
    let mut fixture = started_with(config, FakeRoster::new(two_entries()));
    let source = key(&fixture.roster, "12");
    let destination = key(&fixture.roster, "34");
    let first = record_at(&mut fixture, "12", at(10, 30, 0));
    let second = record_at(&mut fixture, "12", at(10, 32, 0));
    let third = record_at(&mut fixture, "34", at(10, 34, 0));

    // Act
    fixture
        .engine
        .reassign_crossing(2, "34", "mis-keyed plate")
        .expect("the ordinal names a crossing");

    // Assert
    let source_laps = laps_of(&fixture, &source);
    assert_eq!(seqs(&source_laps), vec![1]);
    assert_eq!(source_laps[0].crossed_at, at(10, 30, 0));
    let destination_record: Vec<(u32, Timestamp)> = fixture
        .engine
        .crossings()
        .iter()
        .filter(|crossing| crossing.entry_id == destination)
        .map(|crossing| (crossing.seq, crossing.crossed_at))
        .collect();
    assert_eq!(
        destination_record,
        vec![(1, at(10, 34, 0)), (2, at(10, 32, 0))]
    );
    assert_eq!(
        fixture.engine.credited_cards(&source),
        vec![first.card.expect("the first lap deals a card")]
    );
    assert_eq!(
        fixture.engine.credited_cards(&destination),
        vec![
            third.card.expect("the third lap deals a card"),
            second.card.expect("the second lap deals a card"),
        ]
    );
}

#[test]
fn test_reassign_crossing_renumbers_the_source_later_laps() {
    // Arrange
    let config = min_lap_config(1);
    let mut fixture = started_with(config, FakeRoster::new(two_entries()));
    let source = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let _ = record_at(&mut fixture, "12", at(10, 32, 0));
    let _ = record_at(&mut fixture, "12", at(10, 34, 0));

    // Act
    fixture
        .engine
        .reassign_crossing(1, "34", "mis-keyed plate")
        .expect("the ordinal names a crossing");

    // Assert
    let survivors = laps_of(&fixture, &source);
    assert_eq!(seqs(&survivors), vec![1, 2]);
    assert_eq!(
        survivors
            .iter()
            .map(|crossing| crossing.crossed_at)
            .collect::<Vec<_>>(),
        vec![at(10, 32, 0), at(10, 34, 0)]
    );
}

#[test]
fn test_reassign_crossing_appends_after_the_destinations_laps() {
    // Arrange
    let config = min_lap_config(1);
    let mut fixture = started_with(config, FakeRoster::new(two_entries()));
    let destination = key(&fixture.roster, "34");
    let _ = record_at(&mut fixture, "34", at(10, 30, 0));
    let _ = record_at(&mut fixture, "12", at(10, 32, 0));

    // Act
    fixture
        .engine
        .reassign_crossing(2, "34", "mis-keyed plate")
        .expect("the ordinal names a crossing");

    // Assert
    let arrivals = laps_of(&fixture, &destination);
    assert_eq!(seqs(&arrivals), vec![1, 2]);
    assert_eq!(
        rider_plates(&arrivals),
        vec![Some("34".to_owned()), Some("34".to_owned())]
    );
}

#[test]
fn test_reassign_crossing_keeps_a_held_card_held() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let destination = key(&fixture.roster, "34");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let _ = record_at(&mut fixture, "12", at(10, 0, 30));
    let _ = record_at(&mut fixture, "34", at(10, 34, 0));
    let held_before = fixture.engine.held_crossings()[0].clone();

    // Act
    fixture
        .engine
        .reassign_crossing(2, "34", "mis-keyed plate")
        .expect("the ordinal names a crossing");

    // Assert
    let held = fixture.engine.held_crossings();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].crossing.entry_id, destination);
    assert_eq!(held[0].card, held_before.card);
}

#[test]
fn test_reassign_crossing_keeps_a_void_card_card_voided() {
    // Arrange
    let config = min_lap_config(1);
    let mut fixture = started_with(config, FakeRoster::new(two_entries()));
    let source = key(&fixture.roster, "12");
    let destination = key(&fixture.roster, "34");
    let first = record_at(&mut fixture, "12", at(10, 30, 0));
    let second = record_at(&mut fixture, "12", at(10, 32, 0));
    let voided = dealt_card(&fixture, &last_crossing(&fixture));
    fixture
        .engine
        .void_card("12", &voided, "wrong card dealt")
        .expect("the card is credited");

    // Act
    fixture
        .engine
        .reassign_crossing(2, "34", "mis-keyed plate")
        .expect("the ordinal names a crossing");

    // Assert
    assert_eq!(laps_of(&fixture, &destination).len(), 1);
    assert_eq!(fixture.engine.credited_cards(&destination), vec![]);
    assert_eq!(
        fixture.engine.credited_cards(&source),
        vec![first.card.expect("the first lap deals a card")]
    );
    assert_eq!(
        fixture.engine.is_card_voided(&last_crossing(&fixture)),
        true
    );
    assert_eq!(second.card, Some(voided));
}

#[test]
fn test_reassign_crossing_keeps_a_void_held_card_voided() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let destination = key(&fixture.roster, "34");
    let first = record_at(&mut fixture, "12", at(10, 30, 0));
    let _ = record_at(&mut fixture, "12", at(10, 0, 30));
    let held = fixture.engine.held_crossings()[0].clone();
    fixture
        .engine
        .void_held(&held.crossing)
        .expect("the card is held");

    // Act
    fixture
        .engine
        .reassign_crossing(2, "34", "mis-keyed plate")
        .expect("the ordinal names a crossing");

    // Assert
    assert_eq!(laps_of(&fixture, &destination).len(), 1);
    assert_eq!(fixture.engine.credited_cards(&destination), vec![]);
    assert_eq!(
        fixture.engine.credited_cards(&key(&fixture.roster, "12")),
        vec![first.card.expect("the first lap deals a card")]
    );
}

#[test]
fn test_reassign_crossing_with_an_empty_reason_is_refused() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture.engine.reassign_crossing(1, "34", "");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_reassign_crossing_with_a_whitespace_reason_is_refused() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture.engine.reassign_crossing(1, "34", "  \t");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_reassign_crossing_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture.engine.reassign_crossing(1, "34", "mis-keyed plate");

    // Assert
    assert_eq!(result, Err(illegal("cannot reassign crossing from draft")));
}

#[test]
fn test_reassign_crossing_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let result = fixture.engine.reassign_crossing(1, "34", "mis-keyed plate");

    // Assert
    assert_eq!(
        result,
        Err(illegal("cannot reassign crossing from finished"))
    );
}

#[test]
fn test_reassign_crossing_with_an_unknown_plate_is_refused() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture
        .engine
        .reassign_crossing(1, "999", "mis-keyed plate");

    // Assert
    assert_eq!(
        result,
        Err(RideError::UnknownPlate {
            plate: "999".to_owned(),
        })
    );
}

#[test]
fn test_reassign_crossing_at_ordinal_zero_is_refused() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture.engine.reassign_crossing(0, "34", "mis-keyed plate");

    // Assert
    assert_eq!(result, Err(illegal("no crossing at ordinal 0")));
}

#[test]
fn test_reassign_crossing_at_an_unknown_ordinal_is_refused() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let result = fixture.engine.reassign_crossing(9, "34", "mis-keyed plate");

    // Assert
    assert_eq!(result, Err(illegal("no crossing at ordinal 9")));
}

#[test]
fn test_reassign_crossing_without_a_dealt_card_is_refused() {
    // Arrange
    let mut fixture = running_entries(vec![solo_entry("12"), solo_entry("34")]);
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let crossing = last_crossing(&fixture);
    fixture.engine.dealt.remove(&crossing.deal_seq);

    // Act
    let result = fixture.engine.reassign_crossing(1, "34", "mis-keyed plate");

    // Assert
    assert_eq!(result, Err(illegal("crossing has no dealt card")));
}

// ----------------------------------------------------------------- mark dnf

#[test]
fn test_mark_dnf_of_a_solo_entry_marks_the_whole_entry() {
    // Arrange
    let mut fixture = running_fixture();
    let before = fixture.engine.events().len();

    // Act
    let event = fixture
        .engine
        .mark_dnf("12", "mechanical failure")
        .expect("the ride is running");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::Dnf,
            payload: json!({
                "entry_id": "key-12",
                "plate": "12",
                "rider": false,
                "reason": "mechanical failure",
                "display": "12 · Rider 12",
            }),
        }
    );
    assert_eq!(fixture.engine.events().len(), before + 1);
}

#[test]
fn test_mark_dnf_of_a_solo_entry_calls_set_entry_dnf_with_the_reason() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    fixture
        .engine
        .mark_dnf("12", "mechanical failure")
        .expect("the ride is running");

    // Assert
    assert_eq!(
        fixture.roster.dnf_writes(),
        vec![DnfWrite {
            id: EntryId("key-12".to_owned()),
            plate: "12".to_owned(),
            rider: false,
            reason: "mechanical failure".to_owned(),
        }]
    );
    assert_eq!(fixture.engine.dnf_riders.len(), 0);
}

#[test]
fn test_mark_dnf_of_a_pooled_rider_marks_only_that_rider() {
    // Arrange
    let mut fixture = running_entries(vec![pooled_team()]);

    // Act
    let event = fixture
        .engine
        .mark_dnf("45", "mechanical failure")
        .expect("the ride is running");

    // Assert
    assert_eq!(
        event.payload,
        json!({
            "entry_id": "key-team",
            "plate": "45",
            "rider": true,
            "reason": "mechanical failure",
            "display": "45 · Sarah",
        })
    );
    assert_eq!(fixture.engine.dnf_riders.len(), 1);
    assert_eq!(fixture.engine.dnf_riders.contains("45"), true);
    assert_eq!(fixture.roster.dnf_writes(), vec![]);
}

#[test]
fn test_mark_dnf_of_a_pooled_team_plate_scopes_to_its_member() {
    // Arrange
    let mut fixture = running_entries(vec![pooled_team()]);

    // Act
    let event = fixture
        .engine
        .mark_dnf("9", "mechanical failure")
        .expect("the ride is running");

    // Assert
    assert_eq!(event.payload.get("rider"), Some(&json!(true)));
    assert_eq!(event.payload.get("display"), Some(&json!("9 · Priya")));
    assert_eq!(fixture.engine.dnf_riders.contains("9"), true);
}

#[test]
fn test_mark_dnf_of_a_relay_team_marks_the_whole_entry() {
    // Arrange
    let mut fixture = running_entries(vec![relay_team()]);

    // Act
    let event = fixture
        .engine
        .mark_dnf("77", "mechanical failure")
        .expect("the ride is running");

    // Assert
    assert_eq!(
        event.payload,
        json!({
            "entry_id": "key-relay",
            "plate": "77",
            "rider": false,
            "reason": "mechanical failure",
            "display": "77 · Road Crew",
        })
    );
    assert_eq!(fixture.roster.dnf_writes().len(), 1);
    assert_eq!(fixture.engine.dnf_riders.len(), 0);
}

#[test]
fn test_mark_dnf_of_a_whole_entry_keeps_the_rider_set_empty() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    fixture
        .engine
        .mark_dnf("12", "mechanical failure")
        .expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.dnf_riders.len(), 0);
}

#[test]
fn test_mark_dnf_with_an_empty_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.mark_dnf("12", "");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_mark_dnf_with_a_whitespace_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.mark_dnf("12", " \n");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_mark_dnf_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture.engine.mark_dnf("12", "mechanical failure");

    // Assert
    assert_eq!(result, Err(illegal("cannot mark DNF from draft")));
}

#[test]
fn test_mark_dnf_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();

    // Act
    let result = fixture.engine.mark_dnf("12", "mechanical failure");

    // Assert
    assert_eq!(result, Err(illegal("cannot mark DNF from finished")));
}

#[test]
fn test_mark_dnf_with_an_unknown_plate_is_refused() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let result = fixture.engine.mark_dnf("999", "mechanical failure");

    // Assert
    assert_eq!(
        result,
        Err(RideError::UnknownPlate {
            plate: "999".to_owned(),
        })
    );
}

// --------------------------------------------------------------- void card

#[test]
fn test_void_card_audits_the_entry_card_and_reason() {
    // Arrange
    let mut fixture = running_fixture();
    let result = record_at(&mut fixture, "12", at(10, 30, 0));
    let card = result.card.expect("the crossing deals a card");
    let before = fixture.engine.events().len();

    // Act
    let event = fixture
        .engine
        .void_card("12", &card, "wrong card dealt")
        .expect("the card is credited");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::VoidCard,
            payload: json!({
                "entry_id": "key-12",
                "card": card.code(),
                "reason": "wrong card dealt",
            }),
        }
    );
    assert_eq!(fixture.engine.events().len(), before + 1);
}

#[test]
fn test_void_card_removes_the_card_and_keeps_the_lap() {
    // Arrange
    let config = min_lap_config(1);
    let mut fixture = started_with(config, FakeRoster::new(two_entries()));
    let entry = key(&fixture.roster, "12");
    let first = record_at(&mut fixture, "12", at(10, 30, 0));
    let second = record_at(&mut fixture, "12", at(10, 32, 0));

    // Act
    fixture
        .engine
        .void_card(
            "12",
            &second.card.expect("the crossing deals a card"),
            "wrong card dealt",
        )
        .expect("the card is credited");

    // Assert
    assert_eq!(seqs(fixture.engine.crossings()), vec![1, 2]);
    assert_eq!(
        fixture.engine.credited_cards(&entry),
        vec![first.card.expect("the first lap deals a card")]
    );
}

#[test]
fn test_void_card_marks_the_deal_slot_voided() {
    // Arrange
    let mut fixture = running_fixture();
    let entry = key(&fixture.roster, "12");
    let result = record_at(&mut fixture, "12", at(10, 30, 0));
    let crossing = last_crossing(&fixture);

    // Act
    fixture
        .engine
        .void_card(
            "12",
            &result.card.expect("the crossing deals a card"),
            "wrong card dealt",
        )
        .expect("the card is credited");

    // Assert
    assert_eq!(fixture.engine.is_card_voided(&crossing), true);
    assert_eq!(fixture.engine.credited_cards(&entry), vec![]);
}

#[test]
fn test_void_card_of_a_held_card_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let result = record_at(&mut fixture, "12", at(10, 0, 30));
    assert_eq!(result.flagged, true);

    // Act
    let outcome = fixture.engine.void_card(
        "12",
        &result.card.expect("the crossing deals a card"),
        "wrong card dealt",
    );

    // Assert
    assert_eq!(
        outcome,
        Err(illegal(
            "card is held; confirm or void it through the review panel"
        ))
    );
}

#[test]
fn test_void_card_ignores_an_unrelated_held_card() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = record_at(&mut fixture, "12", at(10, 0, 30));
    let held_before = fixture.engine.held_crossings()[0].clone();
    let _ = record_at(&mut fixture, "12", at(10, 40, 0));
    let credited = dealt_card(&fixture, &last_crossing(&fixture));

    // Act
    fixture
        .engine
        .void_card("12", &credited, "wrong card dealt")
        .expect("the card is credited");

    // Assert
    assert!(
        !fixture
            .engine
            .credited_cards(&key(&fixture.roster, "12"))
            .contains(&credited)
    );
    let held = fixture.engine.held_crossings();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].card, held_before.card);
}

#[test]
fn test_void_card_twice_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let result = record_at(&mut fixture, "12", at(10, 30, 0));
    let card = result.card.expect("the crossing deals a card");
    fixture
        .engine
        .void_card("12", &card, "wrong card dealt")
        .expect("the card is credited");

    // Act
    let outcome = fixture.engine.void_card("12", &card, "wrong card dealt");

    // Assert
    assert_eq!(
        outcome,
        Err(illegal(&format!(
            "no dealt card {} credited to 12",
            card.code()
        )))
    );
}

#[test]
fn test_void_card_of_an_uncredited_card_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let result = record_at(&mut fixture, "12", at(10, 30, 0));
    let card = result.card.expect("the crossing deals a card");

    // Act
    let outcome = fixture.engine.void_card("34", &card, "wrong card dealt");

    // Assert
    assert_eq!(
        outcome,
        Err(illegal(&format!(
            "no dealt card {} credited to 34",
            card.code()
        )))
    );
}

#[test]
fn test_void_card_with_an_empty_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let result = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let outcome =
        fixture
            .engine
            .void_card("12", &result.card.expect("the crossing deals a card"), "");

    // Assert
    assert_eq!(outcome, Err(RideError::EmptyReason));
}

#[test]
fn test_void_card_with_a_whitespace_reason_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let result = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let outcome =
        fixture
            .engine
            .void_card("12", &result.card.expect("the crossing deals a card"), " ");

    // Assert
    assert_eq!(outcome, Err(RideError::EmptyReason));
}

#[test]
fn test_void_card_from_draft_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    let card = reference_card(fixture.engine.config(), 0);

    // Act
    let outcome = fixture.engine.void_card("12", &card, "wrong card dealt");

    // Assert
    assert_eq!(outcome, Err(illegal("cannot void card from draft")));
}

#[test]
fn test_void_card_from_finished_is_refused() {
    // Arrange
    let mut fixture = finished_fixture();
    let card = reference_card(fixture.engine.config(), 0);

    // Act
    let outcome = fixture.engine.void_card("12", &card, "wrong card dealt");

    // Assert
    assert_eq!(outcome, Err(illegal("cannot void card from finished")));
}

#[test]
fn test_void_card_with_an_unknown_plate_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let result = record_at(&mut fixture, "12", at(10, 30, 0));

    // Act
    let outcome = fixture.engine.void_card(
        "999",
        &result.card.expect("the crossing deals a card"),
        "wrong card dealt",
    );

    // Assert
    assert_eq!(
        outcome,
        Err(RideError::UnknownPlate {
            plate: "999".to_owned(),
        })
    );
}

// ------------------------------------------------ pooled rider moves (R-17)

#[test]
fn test_move_rider_team_to_team_reattributes_the_riders_live_crossings() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "1", at(10, 30, 0));
    let _ = record_at(&mut fixture, "2", at(11, 0, 0));
    let _ = record_at(&mut fixture, "2", at(11, 30, 0));
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    let mapped: Vec<(EntryId, Option<String>)> = fixture
        .engine
        .crossings()
        .iter()
        .map(|crossing| (crossing.entry_id.clone(), crossing.rider_plate.clone()))
        .collect();
    assert_eq!(
        mapped,
        vec![
            (alpha, Some("1".to_owned())),
            (beta.clone(), Some("2".to_owned())),
            (beta, Some("2".to_owned())),
        ]
    );
}

#[test]
fn test_move_rider_team_to_team_renumbers_the_source_later_laps() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    let _ = record_at(&mut fixture, "2", at(11, 0, 0));
    let _ = record_at(&mut fixture, "1", at(11, 30, 0));
    let _ = record_at(&mut fixture, "1", at(12, 0, 0));
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    let survivors = laps_of(&fixture, &alpha);
    assert_eq!(seqs(&survivors), vec![1, 2]);
    assert_eq!(
        rider_plates(&survivors),
        vec![Some("1".to_owned()), Some("1".to_owned())]
    );
}

#[test]
fn test_move_rider_team_to_team_appends_after_the_destinations_laps() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "3", at(10, 30, 0));
    let _ = record_at(&mut fixture, "2", at(11, 0, 0));
    let _ = record_at(&mut fixture, "2", at(11, 30, 0));
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    let arrivals: Vec<(u32, Option<String>)> = laps_of(&fixture, &beta)
        .iter()
        .map(|crossing| (crossing.seq, crossing.rider_plate.clone()))
        .collect();
    assert_eq!(
        arrivals,
        vec![
            (1, Some("3".to_owned())),
            (2, Some("2".to_owned())),
            (3, Some("2".to_owned())),
        ]
    );
}

#[test]
fn test_move_rider_team_to_team_audits_both_keys() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    fixture.engine.stop().expect("the ride is running");
    let before = fixture.engine.events().len();

    // Act
    let event = fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::MoveRider,
            payload: json!({
                "rider_plate": "2",
                "from_key": "key-team-a",
                "to_team": "Team B",
                "to_key": "key-team-b",
                "reason": "rider swapped teams",
            }),
        }
    );
    assert_eq!(fixture.engine.events().len(), before + 1);
    assert_eq!(
        fixture.roster.moves(),
        vec![(
            "2".to_owned(),
            EntryId("key-team-b".to_owned()),
            "rider swapped teams".to_owned(),
        )]
    );
}

#[test]
fn test_move_rider_team_to_team_moves_a_rider_without_laps() {
    // Arrange
    let mut fixture = pooled_stopped(two_team_pooled());
    let beta = key(&fixture.roster, "3");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(fixture.engine.crossings(), Vec::new());
    assert_eq!(fixture.engine.credited_cards(&beta), vec![]);
    assert_eq!(
        fixture.roster.resolve_plate("2").map(|entry| entry.id),
        Some(beta)
    );
    assert_eq!(fixture.roster.plates(), vec!["1", "2"]);
}

#[test]
fn test_move_rider_team_to_team_leaves_other_members_data_alone() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "1", at(10, 30, 0));
    let _ = record_at(&mut fixture, "2", at(11, 0, 0));
    let stay_card = dealt_card(&fixture, &fixture.engine.crossings()[0]);
    let move_card = dealt_card(&fixture, &fixture.engine.crossings()[1]);
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(
        rider_plates(&laps_of(&fixture, &alpha)),
        vec![Some("1".to_owned())]
    );
    assert_eq!(fixture.engine.credited_cards(&alpha), vec![stay_card]);
    assert_eq!(fixture.engine.credited_cards(&beta), vec![move_card]);
}

#[test]
fn test_move_rider_team_to_team_moves_credited_cards_with_their_tag() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    let _ = record_at(&mut fixture, "1", at(11, 0, 0));
    let _ = record_at(&mut fixture, "2", at(11, 30, 0));
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(
        fixture.engine.hand.get(&beta),
        Some(&vec![
            (0_u64, Some("2".to_owned())),
            (2_u64, Some("2".to_owned())),
        ])
    );
}

#[test]
fn test_move_rider_team_to_team_keeps_a_manual_card_with_its_source() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    let crossing_card = dealt_card(&fixture, &last_crossing(&fixture));
    fixture
        .engine
        .deal_manual("2", Some("bonus card"))
        .expect("the ride is running");
    let manual_card = fixture.engine.credited_cards(&alpha)[1].clone();
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(fixture.engine.credited_cards(&alpha), vec![manual_card]);
    assert_eq!(fixture.engine.credited_cards(&beta), vec![crossing_card]);
}

#[test]
fn test_move_rider_team_to_team_moves_a_held_card_still_held() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "2", at(10, 0, 30));
    let held_card = fixture.engine.held_crossings()[0].card.clone();
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    let held = fixture.engine.held_crossings();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].crossing.entry_id, beta);
    assert_eq!(held[0].crossing.rider_plate, Some("2".to_owned()));
    assert_eq!(held[0].card, held_card);
    assert_eq!(
        fixture.engine.credited_cards(&key(&fixture.roster, "3")),
        vec![]
    );
}

#[test]
fn test_move_rider_team_to_team_leaves_a_void_card_voided() {
    // Arrange
    let config = min_lap_config(1);
    let mut fixture = pooled_running_with(config, two_team_pooled());
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    let card = dealt_card(&fixture, &last_crossing(&fixture));
    fixture
        .engine
        .void_card("2", &card, "wrong card dealt")
        .expect("the card is credited");
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    let moved = fixture.engine.crossings()[0].clone();
    assert_eq!(moved.entry_id, beta);
    assert_eq!(fixture.engine.card_for(&moved), Some(card));
    assert_eq!(fixture.engine.is_card_voided(&moved), true);
    assert_eq!(fixture.engine.credited_cards(&beta), vec![]);
}

#[test]
fn test_move_rider_team_to_team_restores_a_voided_crossing() {
    // Arrange
    let config = min_lap_config(1);
    let mut fixture = pooled_running_with(config, two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    let first = dealt_card(&fixture, &fixture.engine.crossings()[0]);
    let _ = record_at(&mut fixture, "2", at(11, 0, 0));
    let voided_crossing = last_crossing(&fixture);
    let voided_card = dealt_card(&fixture, &voided_crossing);
    fixture
        .engine
        .void_crossing(&alpha, 2, "double entry")
        .expect("the crossing is live");
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    let restored: Vec<(u32, Timestamp)> = laps_of(&fixture, &beta)
        .iter()
        .map(|crossing| (crossing.seq, crossing.crossed_at))
        .collect();
    assert_eq!(restored, vec![(1, at(10, 30, 0)), (2, at(11, 0, 0))]);
    assert_eq!(fixture.engine.is_card_voided(&voided_crossing), false);
    assert_eq!(
        fixture.engine.credited_cards(&beta),
        vec![first, voided_card]
    );
    assert!(fixture.engine.voided.is_empty());
}

#[test]
fn test_move_rider_team_to_team_reholds_a_restored_short_lap() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    let first = dealt_card(&fixture, &fixture.engine.crossings()[0]);
    let _ = record_at(&mut fixture, "2", at(10, 35, 0));
    let short_card = dealt_card(&fixture, &fixture.engine.crossings()[1]);
    fixture
        .engine
        .void_crossing(&alpha, 2, "double entry")
        .expect("the crossing is live");
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    let held = fixture.engine.held_crossings();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].crossing.entry_id, beta);
    assert_eq!(held[0].card, short_card);
    assert_eq!(fixture.engine.credited_cards(&beta), vec![first]);
}

#[test]
fn test_move_rider_team_to_team_credits_a_restored_short_lap_with_hold_off() {
    // Arrange
    let config = config_with(RideConfigParams {
        hold_short_laps: false,
        ..params()
    });
    let mut fixture = pooled_running_with(config, two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    let first = dealt_card(&fixture, &fixture.engine.crossings()[0]);
    let _ = record_at(&mut fixture, "2", at(10, 35, 0));
    let short_card = dealt_card(&fixture, &fixture.engine.crossings()[1]);
    fixture
        .engine
        .void_crossing(&alpha, 2, "double entry")
        .expect("the crossing is live");
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(fixture.engine.held_crossings(), vec![]);
    assert_eq!(
        fixture.engine.credited_cards(&beta),
        vec![first, short_card]
    );
}

#[test]
fn test_move_rider_team_to_team_leaves_another_riders_voided_lap_alone() {
    // Arrange
    let config = min_lap_config(1);
    let mut fixture = pooled_running_with(config, two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let _ = record_at(&mut fixture, "1", at(10, 30, 0));
    let stay_crossing = last_crossing(&fixture);
    let _ = record_at(&mut fixture, "2", at(11, 0, 0));
    let move_card = dealt_card(&fixture, &last_crossing(&fixture));
    fixture
        .engine
        .void_crossing(&alpha, 1, "double entry")
        .expect("the crossing is live");
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(fixture.engine.is_card_voided(&stay_crossing), true);
    assert_eq!(fixture.engine.voided.len(), 1);
    assert_eq!(fixture.engine.credited_cards(&alpha), vec![]);
    assert_eq!(
        fixture.engine.credited_cards(&key(&fixture.roster, "3")),
        vec![move_card]
    );
}

#[test]
fn test_move_rider_of_the_anchor_riders_keeps_the_stable_keys() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "1", at(10, 30, 0));
    let _ = record_at(&mut fixture, "2", at(11, 0, 0));
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("1", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(fixture.roster.plates(), vec!["2", "1"]);
    let mapped: Vec<(EntryId, Option<String>)> = fixture
        .engine
        .crossings()
        .iter()
        .map(|crossing| (crossing.entry_id.clone(), crossing.rider_plate.clone()))
        .collect();
    assert_eq!(
        mapped,
        vec![(alpha, Some("2".to_owned())), (beta, Some("1".to_owned()))]
    );
}

// ---------------------------------------------------------- solo to team

#[test]
fn test_move_rider_solo_to_team_moves_the_solo_laps_onto_the_team() {
    // Arrange
    let mut fixture = running_entries(solo_and_team());
    let solo = key(&fixture.roster, "5");
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "5", at(10, 30, 0));
    let first = dealt_card(&fixture, &fixture.engine.crossings()[0]);
    let _ = record_at(&mut fixture, "5", at(11, 0, 0));
    let second = dealt_card(&fixture, &fixture.engine.crossings()[1]);
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("5", "Team B", "rider joins a team")
        .expect("the ride is stopped");

    // Assert
    let arrivals: Vec<(EntryId, u32)> = laps_of(&fixture, &beta)
        .iter()
        .map(|crossing| (crossing.entry_id.clone(), crossing.seq))
        .collect();
    assert_eq!(arrivals, vec![(beta.clone(), 1), (beta, 2)]);
    assert_eq!(laps_of(&fixture, &solo), vec![]);
    assert_eq!(
        fixture.engine.credited_cards(&key(&fixture.roster, "3")),
        vec![first, second]
    );
}

#[test]
fn test_move_rider_solo_to_team_audits_the_dissolved_solo_key() {
    // Arrange
    let mut fixture = running_entries(solo_and_team());
    let _ = record_at(&mut fixture, "5", at(10, 30, 0));
    fixture.engine.stop().expect("the ride is running");

    // Act
    let event = fixture
        .engine
        .move_rider("5", "Team B", "rider joins a team")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::MoveRider,
            payload: json!({
                "rider_plate": "5",
                "from_key": "key-5",
                "to_team": "Team B",
                "to_key": "key-team-b",
                "reason": "rider joins a team",
            }),
        }
    );
}

#[test]
fn test_move_rider_solo_to_team_dissolves_the_solo_entry() {
    // Arrange
    let mut fixture = running_entries(solo_and_team());
    let solo = key(&fixture.roster, "5");
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("5", "Team B", "rider joins a team")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(fixture.roster.plates(), vec!["3"]);
    assert_eq!(fixture.roster.entry(&solo), None);
    assert_eq!(
        fixture.roster.resolve_plate("5").map(|entry| entry.id),
        Some(key(&fixture.roster, "3"))
    );
}

// ---------------------------------------------------------- team to solo

#[test]
fn test_extract_rider_to_solo_moves_the_laps_to_the_new_key() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let _ = record_at(&mut fixture, "1", at(10, 30, 0));
    let stay_card = dealt_card(&fixture, &fixture.engine.crossings()[0]);
    let _ = record_at(&mut fixture, "2", at(11, 0, 0));
    let moved_first = dealt_card(&fixture, &fixture.engine.crossings()[1]);
    let _ = record_at(&mut fixture, "2", at(11, 30, 0));
    let moved_second = dealt_card(&fixture, &fixture.engine.crossings()[2]);
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .extract_rider_to_solo("2", "rider rides alone")
        .expect("the ride is stopped");

    // Assert
    let solo = key(&fixture.roster, "2");
    let mapped: Vec<(EntryId, Option<String>)> = fixture
        .engine
        .crossings()
        .iter()
        .map(|crossing| (crossing.entry_id.clone(), crossing.rider_plate.clone()))
        .collect();
    assert_eq!(
        mapped,
        vec![
            (alpha.clone(), Some("1".to_owned())),
            (solo.clone(), Some("2".to_owned())),
            (solo.clone(), Some("2".to_owned())),
        ]
    );
    assert_eq!(
        fixture.engine.credited_cards(&solo),
        vec![moved_first, moved_second]
    );
    assert_eq!(fixture.engine.credited_cards(&alpha), vec![stay_card]);
}

#[test]
fn test_extract_rider_to_solo_audits_both_keys() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    fixture.engine.stop().expect("the ride is running");

    // Act
    let event = fixture
        .engine
        .extract_rider_to_solo("2", "rider rides alone")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(
        event,
        Event {
            action: EventAction::ExtractRiderToSolo,
            payload: json!({
                "rider_plate": "2",
                "from_key": "key-team-a",
                "to_key": "key-solo-2",
                "reason": "rider rides alone",
            }),
        }
    );
    assert_eq!(
        fixture.roster.extracts(),
        vec![("2".to_owned(), "rider rides alone".to_owned())]
    );
}

#[test]
fn test_extract_rider_to_solo_leaves_the_other_member_behind() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let _ = record_at(&mut fixture, "1", at(10, 30, 0));
    let stay_card = dealt_card(&fixture, &fixture.engine.crossings()[0]);
    let _ = record_at(&mut fixture, "2", at(11, 0, 0));
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .extract_rider_to_solo("2", "rider rides alone")
        .expect("the ride is stopped");

    // Assert
    let survivors: Vec<(u32, Option<String>)> = laps_of(&fixture, &alpha)
        .iter()
        .map(|crossing| (crossing.seq, crossing.rider_plate.clone()))
        .collect();
    assert_eq!(survivors, vec![(1, Some("1".to_owned()))]);
    assert_eq!(fixture.engine.credited_cards(&alpha), vec![stay_card]);
}

// ------------------------------------------------------- move refusals

#[test]
fn test_move_rider_while_running_and_not_stopped_refuses_with_stop_first() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());

    // Act
    let result = fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams");

    // Assert
    assert_eq!(result, Err(illegal("Stop the ride first")));
}

#[test]
fn test_extract_rider_to_solo_while_running_and_not_stopped_refuses() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());

    // Act
    let result = fixture
        .engine
        .extract_rider_to_solo("2", "rider rides alone");

    // Assert
    assert_eq!(result, Err(illegal("Stop the ride first")));
}

#[test]
fn test_move_rider_from_draft_is_refused() {
    // Arrange
    let mut fixture = fixture(fixture_config(), FakeRoster::new(two_team_pooled()));

    // Act
    let result = fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams");

    // Assert
    assert_eq!(result, Err(illegal("cannot move a rider from draft")));
}

#[test]
fn test_extract_rider_to_solo_from_draft_is_refused() {
    // Arrange
    let mut fixture = fixture(fixture_config(), FakeRoster::new(two_team_pooled()));

    // Act
    let result = fixture
        .engine
        .extract_rider_to_solo("2", "rider rides alone");

    // Assert
    assert_eq!(result, Err(illegal("cannot move a rider from draft")));
}

#[test]
fn test_move_rider_from_finished_is_refused() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    fixture.engine.finish(&[]).expect("the ride is running");

    // Act
    let result = fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams");

    // Assert
    assert_eq!(result, Err(illegal("cannot move a rider from finished")));
}

#[test]
fn test_move_rider_on_a_relay_ride_is_refused() {
    // Arrange
    let config = config_with(RideConfigParams {
        plate_model: PlateModel::TeamRelay,
        ..params()
    });
    let mut fixture = stopped_with(config, FakeRoster::new(vec![relay_team()]));

    // Act
    let result = fixture
        .engine
        .move_rider("77", "Road Crew", "rider swapped teams");

    // Assert
    assert_eq!(
        result,
        Err(illegal(
            "rider moves need a rider_pooled ride, not team_relay"
        ))
    );
}

#[test]
fn test_move_rider_onto_the_riders_own_team_is_refused() {
    // Arrange
    let mut fixture = pooled_stopped(two_team_pooled());

    // Act
    let result = fixture
        .engine
        .move_rider("2", "Team A", "rider swapped teams");

    // Assert
    assert_eq!(result, Err(illegal("rider 2 is already on team Team A")));
}

#[test]
fn test_move_rider_with_an_empty_reason_is_refused() {
    // Arrange
    let mut fixture = pooled_stopped(two_team_pooled());

    // Act
    let result = fixture.engine.move_rider("2", "Team B", "");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_move_rider_with_a_whitespace_reason_is_refused() {
    // Arrange
    let mut fixture = pooled_stopped(two_team_pooled());

    // Act
    let result = fixture.engine.move_rider("2", "Team B", "\t\n");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_extract_rider_to_solo_with_an_empty_reason_is_refused() {
    // Arrange
    let mut fixture = pooled_stopped(two_team_pooled());

    // Act
    let result = fixture.engine.extract_rider_to_solo("2", "");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_extract_rider_to_solo_with_a_whitespace_reason_is_refused() {
    // Arrange
    let mut fixture = pooled_stopped(two_team_pooled());

    // Act
    let result = fixture.engine.extract_rider_to_solo("2", "  ");

    // Assert
    assert_eq!(result, Err(RideError::EmptyReason));
}

#[test]
fn test_move_rider_with_an_unknown_rider_plate_is_refused() {
    // Arrange
    let mut fixture = pooled_stopped(two_team_pooled());

    // Act
    let result = fixture
        .engine
        .move_rider("99", "Team B", "rider swapped teams");

    // Assert
    assert_eq!(
        result,
        Err(RideError::UnknownPlate {
            plate: "99".to_owned(),
        })
    );
}

#[test]
fn test_move_rider_with_an_unknown_team_is_refused() {
    // Arrange
    let mut fixture = pooled_stopped(two_team_pooled());

    // Act
    let result = fixture
        .engine
        .move_rider("2", "Team Z", "rider swapped teams");

    // Assert
    assert_eq!(
        result,
        Err(RideError::UnknownTeam {
            name: "Team Z".to_owned(),
        })
    );
}

#[test]
fn test_extract_rider_to_solo_from_a_solo_entry_is_refused() {
    // Arrange
    let mut fixture = pooled_stopped(solo_and_team());

    // Act
    let result = fixture
        .engine
        .extract_rider_to_solo("5", "rider rides alone");

    // Assert
    assert_eq!(
        result,
        Err(RideError::NotPooledTeamMember {
            plate: "5".to_owned(),
        })
    );
}

#[test]
fn test_move_rider_refused_while_running_leaves_the_ride_untouched() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    let crossings_before = fixture.engine.crossings().to_vec();
    let hand_before = fixture.engine.credited_cards(&alpha);
    let events_before = fixture.engine.events().to_vec();

    // Act
    let result = fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams");

    // Assert
    assert_eq!(result, Err(illegal("Stop the ride first")));
    assert_eq!(fixture.engine.crossings(), crossings_before);
    assert_eq!(fixture.engine.credited_cards(&alpha), hand_before);
    assert_eq!(fixture.engine.events(), events_before);
    assert_eq!(fixture.engine.credited_cards(&beta), vec![]);
}

#[test]
fn test_move_rider_stopped_in_running_moves_the_rider() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(
        fixture
            .engine
            .crossings()
            .iter()
            .map(|crossing| (crossing.entry_id.clone(), crossing.seq))
            .collect::<Vec<_>>(),
        vec![(beta, 1)]
    );
}

#[test]
fn test_move_rider_on_a_reopened_ride_moves_the_rider() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    fixture.engine.finish(&[]).expect("the ride is running");
    fixture.engine.reopen().expect("the ride is finished");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is reopened");

    // Assert
    assert_eq!(
        fixture
            .engine
            .crossings()
            .iter()
            .map(|crossing| (crossing.entry_id.clone(), crossing.seq))
            .collect::<Vec<_>>(),
        vec![(beta, 1)]
    );
}

#[test]
fn test_move_rider_with_a_roster_refusal_is_returned() {
    // Arrange
    let mut fixture = stopped_with(
        fixture_config(),
        FakeRoster::refusing_moves(two_team_pooled()),
    );

    // Act
    let result = fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams");

    // Assert
    assert_eq!(
        result,
        Err(RideError::Roster {
            message: "the roster refused the move".to_owned(),
        })
    );
}

#[test]
fn test_extract_rider_to_solo_with_a_roster_refusal_is_returned() {
    // Arrange
    let mut fixture = stopped_with(
        fixture_config(),
        FakeRoster::refusing_moves(two_team_pooled()),
    );

    // Act
    let result = fixture
        .engine
        .extract_rider_to_solo("2", "rider rides alone");

    // Assert
    assert_eq!(
        result,
        Err(RideError::Roster {
            message: "the roster refused the extraction".to_owned(),
        })
    );
}

#[test]
fn test_move_rider_restoring_a_voided_lap_without_a_start_is_refused() {
    // Arrange
    let config = min_lap_config(1);
    let mut fixture = pooled_running_with(config, two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    let _ = record_at(&mut fixture, "2", at(11, 0, 0));
    fixture
        .engine
        .void_crossing(&alpha, 2, "double entry")
        .expect("the crossing is live");
    fixture
        .engine
        .void_crossing(&alpha, 1, "double entry")
        .expect("the crossing is live");
    fixture.engine.stop().expect("the ride is running");
    fixture.engine.actual_start = None;

    // Act
    let result = fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams");

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
}

#[test]
fn test_empty_reason_displays_the_oracle_message() {
    // Arrange / Act
    let message = RideError::EmptyReason.to_string();

    // Assert
    assert_eq!(message, "reason must not be empty");
}

#[test]
fn test_move_rider_team_to_team_leaves_a_teammates_cards_when_the_rider_has_none() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let beta = key(&fixture.roster, "3");
    let _ = record_at(&mut fixture, "1", at(10, 30, 0));
    let stay_card = dealt_card(&fixture, &last_crossing(&fixture));
    fixture.engine.stop().expect("the ride is running");

    // Act
    fixture
        .engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");

    // Assert
    assert_eq!(fixture.engine.credited_cards(&alpha), vec![stay_card]);
    assert_eq!(fixture.engine.credited_cards(&beta), vec![]);
}

// ===========================================================================
// EPIC-4 Phase 4: apply() replay, snapshot(), duplicate crossings, tie-break
// ===========================================================================

/// The oracle's fixed salt for the venue's high-card draw.
const TIEBREAK_XOR: u64 = 0x5F3A_6D1C_9E2B_0478;

fn card(code: &str) -> Card {
    Card::parse(code).expect("the fixture card code parses")
}

fn solo_entry_with_sex(plate: &str, sex: Option<&str>) -> EntrySnapshot {
    let mut entry = solo_entry(plate);
    if let Some(rider) = entry.riders.first_mut() {
        rider.sex = sex.map(str::to_owned);
    }
    entry
}

fn team_with_sexes(sexes: [Option<&str>; 2]) -> EntrySnapshot {
    let mut entry = pooled_team();
    for (rider, sex) in entry.riders.iter_mut().zip(sexes) {
        rider.sex = sex.map(str::to_owned);
    }
    entry
}

fn tied_field(count: usize) -> Vec<EntrySnapshot> {
    (1..=count)
        .map(|number| solo_entry(&number.to_string()))
        .collect()
}

fn draws_by_plate(fixture: &Fixture) -> Vec<(String, Option<Card>)> {
    fixture
        .engine
        .snapshot()
        .into_iter()
        .map(|entry| (entry.plate, entry.tiebreak_card))
        .collect()
}

/// The maximum-card configuration: one-second minimum lap, always deal.
fn capped_config(max_cards: u32) -> RideConfig {
    config_with(RideConfigParams {
        min_lap_s: 1,
        hold_short_laps: false,
        max_cards: Some(max_cards),
        ..params()
    })
}

/// The always-deal configuration: one-second minimum lap, no holds.
fn always_deal_config() -> RideConfig {
    config_with(RideConfigParams {
        min_lap_s: 1,
        hold_short_laps: false,
        ..params()
    })
}

fn replay_fixture(config: RideConfig, entries: Vec<EntrySnapshot>, events: &[Event]) -> Fixture {
    let mut fixture = fixture(config, FakeRoster::new(entries));
    for event in events {
        fixture
            .engine
            .apply(event)
            .expect("the recorded event replays");
    }
    fixture
}

#[derive(Debug, PartialEq)]
struct ObservedEntryState {
    id: EntryId,
    lap_times: Vec<f64>,
    credited: Vec<Card>,
}

#[derive(Debug, PartialEq)]
struct ObservedCrossingState {
    entry_id: EntryId,
    seq: u32,
    crossed_at: Timestamp,
    rider_plate: Option<String>,
    deal_seq: u64,
    card: Option<Card>,
    held: Option<Card>,
    voided: bool,
}

/// Every observable engine reading except the audit event bytes.
#[derive(Debug, PartialEq)]
struct ObservedState {
    state: RideStatus,
    actual_start: Option<Timestamp>,
    stopped: bool,
    closed_elapsed: f64,
    self_test_unverified: bool,
    crossings: Vec<Crossing>,
    pending_misses: Vec<PendingMiss>,
    held: Vec<HeldCrossing>,
    shoe_remaining: usize,
    shoe_total: usize,
    entry_count: usize,
    on_course: usize,
    snapshot: Vec<SnapshotEntry>,
    duplicates: Vec<(Crossing, Crossing)>,
    per_entry: Vec<ObservedEntryState>,
    dispositions: Vec<ObservedCrossingState>,
}

fn observe_state(fixture: &Fixture) -> ObservedState {
    let engine = &fixture.engine;
    let entries = fixture.roster.entries();
    let per_entry = entries
        .iter()
        .map(|entry| ObservedEntryState {
            id: entry.id.clone(),
            lap_times: engine.lap_times(&entry.id),
            credited: engine.credited_cards(&entry.id),
        })
        .collect();
    let dispositions = engine
        .crossings()
        .iter()
        .map(|crossing| ObservedCrossingState {
            entry_id: crossing.entry_id.clone(),
            seq: crossing.seq,
            crossed_at: crossing.crossed_at,
            rider_plate: crossing.rider_plate.clone(),
            deal_seq: crossing.deal_seq,
            card: engine.card_for(crossing),
            held: engine.held_card_for(crossing),
            voided: engine.is_card_voided(crossing),
        })
        .collect();
    ObservedState {
        state: engine.state(),
        actual_start: engine.actual_start(),
        stopped: engine.stopped(),
        closed_elapsed: engine.closed_elapsed(),
        self_test_unverified: engine.self_test_unverified(),
        crossings: engine.crossings().to_vec(),
        pending_misses: engine.pending_misses(),
        held: engine.held_crossings(),
        shoe_remaining: engine.shoe_remaining(),
        shoe_total: engine.shoe_total(),
        entry_count: engine.entry_count(),
        on_course: engine.on_course(),
        snapshot: engine.snapshot(),
        duplicates: engine.duplicate_crossings(),
        per_entry,
        dispositions,
    }
}

// ------------------------------------------------------- apply: lifecycle

#[test]
fn test_apply_start_event_bypasses_the_gates_on_an_empty_roster() {
    // Arrange
    let mut fixture = fixture(fixture_config(), FakeRoster::new(Vec::new()));
    let event = Event {
        action: EventAction::Start,
        payload: json!({
            "actual_start": "2026-09-20T10:00:00",
            "reason": "0:00:00",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.state(), RideStatus::Running);
    assert_eq!(fixture.engine.actual_start(), Some(at(10, 0, 0)));
    assert_eq!(fixture.engine.events().to_vec(), vec![event]);
}

#[test]
fn test_apply_continue_event_keeps_the_actual_start() {
    // Arrange
    let mut fixture = stopped_fixture();
    let event = Event {
        action: EventAction::Continue,
        payload: json!({
            "actual_start": "2026-09-20T10:00:00",
            "reason": "0:00:00",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.state(), RideStatus::Running);
    assert_eq!(fixture.engine.stopped(), false);
    assert_eq!(fixture.engine.actual_start(), Some(at(10, 0, 0)));
    assert_eq!(
        fixture.engine.events().last().map(|event| event.action),
        Some(EventAction::Continue)
    );
}

#[test]
fn test_apply_set_start_time_event_backdates_the_actual_start() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::SetStartTime,
        payload: json!({
            "actual_start": "2026-09-20T09:55:00",
            "previous_start": "2026-09-20T10:00:00",
            "reason": "-0:05:00",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.actual_start(), Some(at(9, 55, 0)));
    assert_eq!(
        fixture.engine.events().last().map(|event| event.action),
        Some(EventAction::SetStartTime)
    );
}

#[test]
fn test_apply_stop_event_sets_the_stop_guard() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::Stop,
        payload: json!({
            "stopped_at": "2026-09-20T10:00:00",
            "reason": "0:00:00",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.state(), RideStatus::Running);
    assert_eq!(fixture.engine.stopped(), true);
}

#[test]
fn test_apply_finish_event_records_the_payload_instant() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::Finish,
        payload: json!({
            "finished_at": "2026-09-20T10:30:00",
            "reason": "0:30:00",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.state(), RideStatus::Finished);
    assert_eq!(fixture.engine.closed_elapsed(), 1_800.0);
}

#[test]
fn test_apply_finish_event_does_not_draw_the_tiebreak() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::Finish,
        payload: json!({
            "finished_at": "2026-09-20T10:30:00",
            "reason": "0:30:00",
        }),
    };

    // Act
    fixture.engine.apply(&event).expect("the finish replays");

    // Assert
    assert_eq!(
        fixture.engine.events().last().map(|event| event.action),
        Some(EventAction::Finish)
    );
    assert_eq!(
        draws_by_plate(&fixture),
        vec![("12".to_owned(), None), ("34".to_owned(), None),]
    );
}

#[test]
fn test_apply_reopen_event_reopens_the_shoe() {
    // Arrange
    let mut fixture = running_fixture();
    fixture
        .engine
        .apply(&Event {
            action: EventAction::Finish,
            payload: json!({
                "finished_at": "2026-09-20T10:30:00",
                "reason": "0:30:00",
            }),
        })
        .expect("the finish replays");
    let event = Event {
        action: EventAction::Reopen,
        payload: json!({
            "reopened_at": "2026-09-20T10:40:00",
            "reason": "0:40:00",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.state(), RideStatus::Reopened);
    assert_eq!(
        fixture
            .engine
            .deal_manual("12", Some("reopened"))
            .map(|event| event.action),
        Ok(EventAction::DealManual)
    );
}

// ------------------------------------------------------- apply: crossings

#[test]
fn test_apply_record_crossing_event_credits_the_lap_and_card() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(two_entries()));
    let key_12 = key(&fixture.roster, "12");
    let event = Event {
        action: EventAction::RecordCrossing,
        payload: json!({
            "plate": "12",
            "entry_id": key_12.0.clone(),
            "lap": 1,
            "crossed_at": "2026-09-20T10:02:00",
            "reason": "Rider 12 · solo",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.lap_times(&key_12), vec![120.0]);
    assert_eq!(fixture.engine.credited_cards(&key_12).len(), 1);
    assert_eq!(
        fixture.engine.events().last().map(|event| event.action),
        Some(EventAction::RecordCrossing)
    );
}

#[test]
fn test_apply_confirm_held_event_releases_the_card() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");
    let event = Event {
        action: EventAction::ConfirmHeld,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": 1,
            "card": held.card.code(),
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(
        fixture
            .engine
            .snapshot()
            .first()
            .map(|entry| entry.cards.clone()),
        Some(vec![held.card.clone()])
    );
}

#[test]
fn test_apply_void_held_event_discards_the_card() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");
    let event = Event {
        action: EventAction::VoidHeld,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": 1,
            "card": held.card.code(),
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.credited_cards(&key_12), Vec::new());
    assert_eq!(fixture.engine.is_card_voided(&held.crossing), true);
}

#[test]
fn test_apply_return_to_held_event_reholds_the_card() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let crossing_result = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let crossing = fixture
        .engine
        .crossings()
        .first()
        .cloned()
        .expect("one crossing is recorded");
    let card = crossing_result.card.expect("the crossing dealt a card");
    let event = Event {
        action: EventAction::ReturnToHeld,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": 1,
            "card": card.code(),
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.held_card_for(&crossing), Some(card));
    assert_eq!(fixture.engine.credited_cards(&key_12), Vec::new());
}

#[test]
fn test_apply_undo_event_reverses_the_last_crossing() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let second = fixture.engine.record_crossing("12", Some(at(10, 32, 0)));
    let event = Event {
        action: EventAction::Undo,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": 2,
            "crossed_at": "2026-09-20T10:32:00",
            "card": second.card.expect("the crossing dealt a card").code(),
            "reason": "Undo last crossing",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.crossings().len(), 1);
    assert_eq!(fixture.engine.lap_times(&key_12), vec![1_800.0]);
}

#[test]
fn test_apply_confirm_held_for_an_unrecorded_crossing_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let event = Event {
        action: EventAction::ConfirmHeld,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": 99,
            "card": "AS",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(
        result,
        Err(RideError::NoCrossing {
            entry_id: key_12.0,
            seq: 99,
        })
    );
}

// ----------------------------------------------------- apply: corrections

#[test]
fn test_apply_deal_manual_event_credits_the_payload_card() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let expected = reference_card(fixture.engine.config(), 0);
    let event = Event {
        action: EventAction::DealManual,
        payload: json!({
            "plate": "12",
            "entry_id": key_12.0.clone(),
            "card": expected.code(),
            "reason": "replacement card",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.credited_cards(&key_12), vec![expected]);
}

#[test]
fn test_apply_edit_crossing_event_retimes_the_lap() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let event = Event {
        action: EventAction::EditCrossing,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": 1,
            "previous_crossed_at": "2026-09-20T10:30:00",
            "crossed_at": "2026-09-20T10:40:00",
            "reason": "clock drift",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.lap_times(&key_12), vec![2_400.0]);
}

#[test]
fn test_apply_void_crossing_event_removes_the_lap() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let _ = fixture.engine.record_crossing("12", Some(at(11, 0, 0)));
    let event = Event {
        action: EventAction::VoidCrossing,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": 1,
            "reason": "double entry",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.crossings().len(), 1);
    assert_eq!(
        fixture
            .engine
            .crossings()
            .first()
            .map(|crossing| crossing.seq),
        Some(1)
    );
}

#[test]
fn test_apply_add_crossing_at_event_records_the_lap() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let event = Event {
        action: EventAction::AddCrossingAt,
        payload: json!({
            "plate": "12",
            "entry_id": key_12.0.clone(),
            "crossed_at": "2026-09-20T11:00:00",
            "reason": "missed at the line",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.crossings().len(), 1);
    assert_eq!(fixture.engine.lap_times(&key_12), vec![3_600.0]);
}

#[test]
fn test_apply_record_miss_event_queues_the_miss() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::RecordMiss,
        payload: json!({
            "miss_seq": 1,
            "crossed_at": "2026-09-20T10:05:00",
            "reason": "missed number",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(
        fixture.engine.pending_misses(),
        vec![PendingMiss {
            miss_seq: 1,
            crossed_at: at(10, 5, 0),
        }]
    );
}

#[test]
fn test_apply_assign_plate_to_miss_event_records_the_crossing() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    fixture
        .engine
        .record_miss(at(10, 5, 0), "missed number")
        .expect("the ride is running");
    let event = Event {
        action: EventAction::AssignPlateToMiss,
        payload: json!({
            "miss_seq": 1,
            "new_plate": "12",
            "entry_id": key_12.0.clone(),
            "crossed_at": "2026-09-20T10:05:00",
            "reason": "rider identified",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.pending_misses(), Vec::new());
    assert_eq!(fixture.engine.crossings().len(), 1);
    assert_eq!(
        fixture
            .engine
            .crossings()
            .first()
            .map(|crossing| crossing.seq),
        Some(1)
    );
}

#[test]
fn test_apply_reassign_event_moves_the_crossing() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let key_34 = key(&fixture.roster, "34");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let event = Event {
        action: EventAction::Reassign,
        payload: json!({
            "seq": 1,
            "old_entry_id": key_12.0.clone(),
            "new_entry_id": key_34.0.clone(),
            "new_plate": "34",
            "reason": "wrong plate at the line",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(
        fixture
            .engine
            .crossings()
            .first()
            .map(|crossing| crossing.entry_id.clone()),
        Some(key_34.clone())
    );
    assert_eq!(fixture.engine.credited_cards(&key_12), Vec::new());
    assert_eq!(fixture.engine.credited_cards(&key_34).len(), 1);
}

#[test]
fn test_apply_dnf_event_marks_the_entry() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let event = Event {
        action: EventAction::Dnf,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "plate": "12",
            "rider": false,
            "reason": "withdrew",
            "display": "12 · Rider 12",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(
        fixture.engine.snapshot().first().map(|entry| entry.dnf),
        Some(true)
    );
}

#[test]
fn test_apply_dnf_event_marks_the_rider_from_the_payload() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let _ = record_at(&mut fixture, "2", at(10, 30, 0));
    let event = Event {
        action: EventAction::Dnf,
        payload: json!({
            "entry_id": alpha.0.clone(),
            "plate": "2",
            "rider": true,
            "reason": "mechanical failure",
            "display": "2 · Bea",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    let team = fixture
        .engine
        .snapshot()
        .into_iter()
        .find(|entry| entry.entry_id == alpha)
        .expect("the team is in the snapshot");
    assert_eq!(team.dnf, false);
    assert_eq!(team.cards, Vec::new());
}

#[test]
fn test_apply_void_card_event_removes_the_card() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let crossing = fixture
        .engine
        .crossings()
        .first()
        .cloned()
        .expect("one crossing is recorded");
    let card = dealt_card(&fixture, &crossing);
    let event = Event {
        action: EventAction::VoidCard,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "card": card.code(),
            "reason": "wrong card dealt",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.credited_cards(&key_12), Vec::new());
    assert_eq!(fixture.engine.is_card_voided(&crossing), true);
}

// ---------------------------------------------------------- apply: moves

#[test]
fn test_apply_move_rider_event_reattributes_the_rider_data() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let beta = key(&fixture.roster, "3");
    let moved = record_at(&mut fixture, "2", at(10, 30, 0));
    fixture.engine.stop().expect("the ride is running");
    fixture
        .roster
        .move_rider("2", &beta, "rider swapped teams")
        .expect("the roster accepts the move");
    let event = Event {
        action: EventAction::MoveRider,
        payload: json!({
            "rider_plate": "2",
            "from_key": alpha.0.clone(),
            "to_team": "Team B",
            "to_key": beta.0.clone(),
            "reason": "rider swapped teams",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.credited_cards(&alpha), Vec::new());
    assert_eq!(
        fixture.engine.credited_cards(&beta),
        vec![moved.card.expect("the crossing dealt a card")]
    );
    assert_eq!(
        fixture
            .engine
            .crossings()
            .first()
            .map(|crossing| crossing.entry_id.clone()),
        Some(beta)
    );
}

#[test]
fn test_apply_extract_rider_to_solo_event_reattributes_the_rider_data() {
    // Arrange
    let mut fixture = running_entries(two_team_pooled());
    let alpha = key(&fixture.roster, "1");
    let moved = record_at(&mut fixture, "2", at(10, 30, 0));
    fixture.engine.stop().expect("the ride is running");
    let solo_key = fixture
        .roster
        .extract_rider_to_solo("2", "rider rides alone")
        .expect("the roster accepts the extraction");
    let event = Event {
        action: EventAction::ExtractRiderToSolo,
        payload: json!({
            "rider_plate": "2",
            "from_key": alpha.0.clone(),
            "to_key": solo_key.0.clone(),
            "reason": "rider rides alone",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.credited_cards(&alpha), Vec::new());
    assert_eq!(
        fixture.engine.credited_cards(&solo_key),
        vec![moved.card.expect("the crossing dealt a card")]
    );
}

// ------------------------------------------- apply: draw, reshuffle, guard

#[test]
fn test_apply_tiebreak_draw_event_restores_the_draws() {
    // Arrange
    let mut fixture = default_fixture();
    let key_12 = key(&fixture.roster, "12");
    let key_34 = key(&fixture.roster, "34");
    let event = Event {
        action: EventAction::TiebreakDraw,
        payload: json!({
            "draws": [
                {"entry_id": key_12.0.clone(), "card": "AS"},
                {"entry_id": key_34.0.clone(), "card": "7C"},
            ],
            "summary": "12 · AS, 34 · 7C",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(
        draws_by_plate(&fixture),
        vec![
            ("12".to_owned(), Some(card("AS"))),
            ("34".to_owned(), Some(card("7C"))),
        ]
    );
    assert_eq!(fixture.engine.events().last(), Some(&event));
}

#[test]
fn test_apply_shoe_reshuffle_event_is_a_no_op() {
    // Arrange
    let mut fixture = default_fixture();
    let events_before = fixture.engine.events().to_vec();
    let remaining_before = fixture.engine.shoe_remaining();
    let event = Event {
        action: EventAction::ShoeReshuffle,
        payload: json!({"cycle": 2, "jokers_added": 1, "reason": "1 jokers added"}),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.events().to_vec(), events_before);
    assert_eq!(fixture.engine.shoe_remaining(), remaining_before);
}

#[test]
fn test_apply_unknown_action_returns_unknown_event_action() {
    // Arrange
    let mut fixture = default_fixture();

    // Act
    let result = fixture.engine.apply_action("bogus_action", &json!({}));

    // Assert
    assert_eq!(
        result,
        Err(RideError::UnknownEventAction {
            action: "bogus_action".to_owned(),
        })
    );
}

#[test]
fn test_apply_record_crossing_with_an_unknown_entry_key_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::RecordCrossing,
        payload: json!({
            "plate": "12",
            "entry_id": "missing-key",
            "lap": 1,
            "crossed_at": "2026-09-20T10:02:00",
            "reason": "Rider 12 · solo",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(
        result,
        Err(RideError::UnknownEntry {
            key: "missing-key".to_owned(),
        })
    );
}

// ------------------------------------------- apply: payload tolerances

#[test]
fn test_apply_finish_event_without_checks_writes_no_key() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::Finish,
        payload: json!({"finished_at": "2026-09-20T10:30:00", "reason": "0:30:00"}),
    };

    // Act
    fixture.engine.apply(&event).expect("the finish replays");

    // Assert
    let last = fixture
        .engine
        .events()
        .last()
        .expect("the finish event is appended");
    assert_eq!(last.payload.get("self_test_failed_checks"), None);
}

#[test]
fn test_apply_finish_event_with_a_non_list_checks_value_writes_no_key() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::Finish,
        payload: json!({
            "finished_at": "2026-09-20T10:30:00",
            "reason": "0:30:00",
            "self_test_failed_checks": "7,462 distinct ranks",
        }),
    };

    // Act
    fixture.engine.apply(&event).expect("the finish replays");

    // Assert
    let last = fixture
        .engine
        .events()
        .last()
        .expect("the finish event is appended");
    assert_eq!(last.payload.get("self_test_failed_checks"), None);
}

#[test]
fn test_apply_finish_event_with_numeric_checks_writes_strings() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::Finish,
        payload: json!({
            "finished_at": "2026-09-20T10:30:00",
            "reason": "0:30:00",
            "self_test_failed_checks": [7462],
        }),
    };

    // Act
    fixture.engine.apply(&event).expect("the finish replays");

    // Assert
    let last = fixture
        .engine
        .events()
        .last()
        .expect("the finish event is appended");
    assert_eq!(
        last.payload.get("self_test_failed_checks"),
        Some(&json!(["7462"]))
    );
}

#[test]
fn test_apply_finish_event_with_string_checks_keeps_them() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::Finish,
        payload: json!({
            "finished_at": "2026-09-20T10:30:00",
            "reason": "0:30:00",
            "self_test_failed_checks": ["7,462 distinct ranks", "compare() total order"],
        }),
    };

    // Act
    fixture.engine.apply(&event).expect("the finish replays");

    // Assert
    let last = fixture
        .engine
        .events()
        .last()
        .expect("the finish event is appended");
    assert_eq!(
        last.payload.get("self_test_failed_checks"),
        Some(&json!(["7,462 distinct ranks", "compare() total order"]))
    );
    assert_eq!(fixture.engine.self_test_unverified(), true);
}

#[test]
fn test_apply_tiebreak_draw_with_an_absent_draws_key_reads_no_draws() {
    // Arrange
    let mut fixture = default_fixture();
    let event = Event {
        action: EventAction::TiebreakDraw,
        payload: json!({"summary": ""}),
    };

    // Act
    fixture.engine.apply(&event).expect("the row replays");
    let draws = draws_by_plate(&fixture);

    // Assert
    assert_eq!(
        draws,
        vec![("12".to_owned(), None), ("34".to_owned(), None)]
    );
}

#[test]
fn test_apply_tiebreak_draw_with_a_null_draws_value_reads_no_draws() {
    // Arrange
    let mut fixture = default_fixture();
    let event = Event {
        action: EventAction::TiebreakDraw,
        payload: json!({"draws": null}),
    };

    // Act
    fixture.engine.apply(&event).expect("the row replays");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture),
        vec![("12".to_owned(), None), ("34".to_owned(), None)]
    );
}

#[test]
fn test_apply_tiebreak_draw_with_a_string_draws_value_reads_no_draws() {
    // Arrange
    let mut fixture = default_fixture();
    let event = Event {
        action: EventAction::TiebreakDraw,
        payload: json!({"draws": "AS"}),
    };

    // Act
    fixture.engine.apply(&event).expect("the row replays");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture),
        vec![("12".to_owned(), None), ("34".to_owned(), None)]
    );
}

#[test]
fn test_apply_tiebreak_draw_with_a_number_draws_value_reads_no_draws() {
    // Arrange
    let mut fixture = default_fixture();
    let event = Event {
        action: EventAction::TiebreakDraw,
        payload: json!({"draws": 7}),
    };

    // Act
    fixture.engine.apply(&event).expect("the row replays");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture),
        vec![("12".to_owned(), None), ("34".to_owned(), None)]
    );
}

#[test]
fn test_apply_tiebreak_draw_with_an_empty_draws_list_reads_no_draws() {
    // Arrange
    let mut fixture = default_fixture();
    let event = Event {
        action: EventAction::TiebreakDraw,
        payload: json!({"draws": []}),
    };

    // Act
    fixture.engine.apply(&event).expect("the row replays");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture),
        vec![("12".to_owned(), None), ("34".to_owned(), None)]
    );
}

// -------------------------------------------------------------- snapshot

#[test]
fn test_snapshot_before_start_lists_each_entry_with_zero_laps() {
    // Arrange
    let fixture = default_fixture();

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results
            .iter()
            .map(|entry| (
                entry.plate.clone(),
                entry.laps,
                entry.total_time,
                entry.best_lap,
                entry.cards.clone(),
                entry.hand.clone(),
                entry.dnf,
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "12".to_owned(),
                0,
                0.0,
                0.0,
                Vec::new(),
                best_hand(&[]),
                false,
            ),
            (
                "34".to_owned(),
                0,
                0.0,
                0.0,
                Vec::new(),
                best_hand(&[]),
                false,
            ),
        ]
    );
}

#[test]
fn test_snapshot_on_an_empty_roster_is_empty() {
    // Arrange
    let fixture = fixture(fixture_config(), FakeRoster::new(Vec::new()));

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(results, Vec::new());
}

#[test]
fn test_snapshot_derives_laps_total_and_best_lap() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 3, 20)));
    let _ = fixture.engine.record_crossing("12", Some(at(10, 5, 20)));

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results
            .iter()
            .map(|entry| (
                entry.plate.clone(),
                entry.laps,
                entry.total_time,
                entry.best_lap
            ))
            .collect::<Vec<_>>(),
        vec![
            ("12".to_owned(), 2, 320.0, 120.0),
            ("34".to_owned(), 0, 0.0, 0.0),
        ]
    );
}

#[test]
fn test_snapshot_pools_the_credited_cards_in_deal_order() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(two_entries()));
    let first = record_at(&mut fixture, "12", at(10, 1, 0));
    let second = record_at(&mut fixture, "12", at(10, 2, 0));

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results.first().map(|entry| entry.cards.clone()),
        Some(vec![
            first.card.expect("the first crossing dealt a card"),
            second.card.expect("the second crossing dealt a card"),
        ])
    );
}

#[test]
fn test_snapshot_excludes_held_cards_from_the_hand() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results
            .first()
            .map(|entry| (entry.cards.clone(), entry.hand.clone())),
        Some((Vec::new(), best_hand(&[])))
    );
}

#[test]
fn test_snapshot_excludes_a_voided_card() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(two_entries()));
    let key_12 = key(&fixture.roster, "12");
    let result = record_at(&mut fixture, "12", at(10, 1, 0));
    let card = result.card.expect("the crossing dealt a card");
    fixture
        .engine
        .void_card("12", &card, "wrong card")
        .expect("the card is credited");

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results
            .iter()
            .find(|entry| entry.entry_id == key_12)
            .map(|entry| entry.cards.clone()),
        Some(Vec::new())
    );
}

#[test]
fn test_snapshot_caps_the_scoring_cards_at_max_cards() {
    // Arrange
    let mut fixture = started_with(capped_config(2), FakeRoster::new(two_entries()));
    let key_12 = key(&fixture.roster, "12");
    let first = record_at(&mut fixture, "12", at(10, 1, 0));
    let second = record_at(&mut fixture, "12", at(10, 2, 0));
    let _ = record_at(&mut fixture, "12", at(10, 3, 0));

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    let entry = results
        .iter()
        .find(|entry| entry.entry_id == key_12)
        .expect("the entry is in the snapshot");
    assert_eq!(
        entry.cards,
        vec![
            first.card.expect("the first crossing dealt a card"),
            second.card.expect("the second crossing dealt a card"),
        ]
    );
    assert_eq!(entry.hand, best_hand(&entry.cards));
}

#[test]
fn test_snapshot_keeps_laps_past_the_card_cap() {
    // Arrange
    let mut fixture = started_with(capped_config(2), FakeRoster::new(two_entries()));
    let key_12 = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 1, 0));
    let _ = record_at(&mut fixture, "12", at(10, 2, 0));
    let _ = record_at(&mut fixture, "12", at(10, 3, 0));

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results
            .iter()
            .find(|entry| entry.entry_id == key_12)
            .map(|entry| entry.laps),
        Some(3)
    );
}

#[test]
fn test_snapshot_marks_a_dnf_entry() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    fixture
        .engine
        .mark_dnf("12", "withdrew")
        .expect("the ride is running");

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results
            .iter()
            .map(|entry| (entry.entry_id.clone(), entry.dnf))
            .collect::<Vec<_>>(),
        vec![(key_12, true), (key(&fixture.roster, "34"), false)]
    );
}

#[test]
fn test_snapshot_forfeits_a_dnf_riders_cards_and_keeps_the_team() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(vec![pooled_team()]));
    let team = key(&fixture.roster, "9");
    let forfeited = record_at(&mut fixture, "45", at(10, 1, 0));
    let kept = record_at(&mut fixture, "9", at(10, 2, 0));
    fixture
        .engine
        .mark_dnf("45", "mechanical failure")
        .expect("the ride is running");

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    let entry = results
        .iter()
        .find(|entry| entry.entry_id == team)
        .expect("the team is in the snapshot");
    assert_eq!(entry.dnf, false);
    assert_eq!(
        entry.cards,
        vec![kept.card.expect("the crossing dealt a card")]
    );
    assert_eq!(entry.hand, best_hand(&entry.cards));
    assert_eq!(
        entry
            .cards
            .contains(&forfeited.card.expect("the crossing dealt a card")),
        false
    );
}

#[test]
fn test_snapshot_marks_a_team_dnf_when_every_rider_is_dnf() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(vec![pooled_team()]));
    let team = key(&fixture.roster, "9");
    let _ = record_at(&mut fixture, "45", at(10, 1, 0));
    fixture
        .engine
        .mark_dnf("45", "mechanical failure")
        .expect("the ride is running");
    fixture
        .engine
        .mark_dnf("9", "mechanical failure")
        .expect("the ride is running");

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    let entry = results
        .iter()
        .find(|entry| entry.entry_id == team)
        .expect("the team is in the snapshot");
    assert_eq!(
        (entry.dnf, entry.cards.clone(), entry.hand.clone()),
        (true, Vec::new(), best_hand(&[]))
    );
}

#[test]
fn test_snapshot_keeps_a_dnf_riders_cards_in_the_credited_read() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(vec![pooled_team()]));
    let team = key(&fixture.roster, "9");
    let result = record_at(&mut fixture, "45", at(10, 1, 0));
    fixture
        .engine
        .mark_dnf("45", "mechanical failure")
        .expect("the ride is running");

    // Act
    let credited = fixture.engine.credited_cards(&team);

    // Assert
    assert_eq!(
        credited,
        vec![result.card.expect("the crossing dealt a card")]
    );
}

#[test]
fn test_snapshot_sets_a_solo_entries_sex() {
    // Arrange
    let fixture = fixture(
        fixture_config(),
        FakeRoster::new(vec![solo_entry_with_sex("12", Some("F"))]),
    );

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results.first().map(|entry| entry.sex.clone()),
        Some(Some("F".to_owned()))
    );
}

#[test]
fn test_snapshot_leaves_a_solo_entries_sex_none() {
    // Arrange
    let fixture = fixture(fixture_config(), FakeRoster::new(vec![solo_entry("12")]));

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(results.first().map(|entry| entry.sex.clone()), Some(None));
}

#[test]
fn test_snapshot_leaves_a_team_entries_sex_none() {
    // Arrange
    let fixture = fixture(
        fixture_config(),
        FakeRoster::new(vec![team_with_sexes([Some("F"), Some("M")])]),
    );

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(results.first().map(|entry| entry.sex.clone()), Some(None));
}

#[test]
fn test_snapshot_reports_the_solo_kind() {
    // Arrange
    let fixture = default_fixture();

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results.iter().map(|entry| entry.kind).collect::<Vec<_>>(),
        vec![SnapshotKind::Solo, SnapshotKind::Solo]
    );
}

#[test]
fn test_snapshot_reports_the_team_kind() {
    // Arrange
    let fixture = fixture(fixture_config(), FakeRoster::new(vec![pooled_team()]));

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results.first().map(|entry| entry.kind),
        Some(SnapshotKind::Team)
    );
}

#[test]
fn test_snapshot_reports_no_tiebreak_card_before_a_finish() {
    // Arrange
    let fixture = default_fixture();

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results
            .iter()
            .map(|entry| entry.tiebreak_card.clone())
            .collect::<Vec<_>>(),
        vec![None, None]
    );
}

#[test]
fn test_snapshot_reports_plate_name_and_entry_id() {
    // Arrange
    let fixture = default_fixture();
    let key_12 = key(&fixture.roster, "12");

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results.first().map(|entry| (
            entry.entry_id.clone(),
            entry.plate.clone(),
            entry.name.clone(),
        )),
        Some((key_12, "12".to_owned(), "Rider 12".to_owned()))
    );
}

#[test]
fn test_snapshot_is_pure_and_repeatable() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let events_before = fixture.engine.events().to_vec();

    // Act
    let first = fixture.engine.snapshot();
    let second = fixture.engine.snapshot();

    // Assert
    assert_eq!(first, second);
    assert_eq!(fixture.engine.events().to_vec(), events_before);
}

// --------------------------------------------------------- entry_is_dnf

#[test]
fn test_entry_is_dnf_given_a_healthy_entry_is_false() {
    // Arrange
    let fixture = fixture(fixture_config(), FakeRoster::new(vec![pooled_team()]));
    let team = fixture
        .roster
        .resolve_plate("9")
        .expect("the team resolves");

    // Act
    let dnf = fixture.engine.entry_is_dnf(&team);

    // Assert
    assert_eq!(dnf, false);
}

#[test]
fn test_entry_is_dnf_given_a_dnf_entry_status_is_true() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    fixture
        .engine
        .mark_dnf("12", "withdrew")
        .expect("the mark is allowed");
    let entry = fixture.roster.entry(&key_12).expect("the entry resolves");

    // Act
    let dnf = fixture.engine.entry_is_dnf(&entry);

    // Assert
    assert_eq!(dnf, true);
}

#[test]
fn test_entry_is_dnf_given_a_pooled_team_with_one_rider_out_is_false() {
    // Arrange
    let mut fixture = running_entries(vec![pooled_team()]);
    let team = fixture
        .roster
        .resolve_plate("9")
        .expect("the team resolves");
    fixture
        .engine
        .mark_dnf("45", "mechanical failure")
        .expect("the mark is allowed");

    // Act
    let dnf = fixture.engine.entry_is_dnf(&team);

    // Assert
    assert_eq!(dnf, false);
}

#[test]
fn test_entry_is_dnf_given_a_pooled_team_all_out_is_true() {
    // Arrange
    let mut fixture = running_entries(vec![pooled_team()]);
    let team = fixture
        .roster
        .resolve_plate("9")
        .expect("the team resolves");
    fixture
        .engine
        .mark_dnf("45", "mechanical failure")
        .expect("the mark is allowed");
    fixture
        .engine
        .mark_dnf("9", "mechanical failure")
        .expect("the mark is allowed");

    // Act
    let dnf = fixture.engine.entry_is_dnf(&team);

    // Assert
    assert_eq!(dnf, true);
}

#[test]
fn test_entry_is_dnf_given_a_riderless_entry_is_false() {
    // Arrange
    let fixture = default_fixture();
    let riderless = EntrySnapshot {
        id: EntryId("key-empty".to_owned()),
        plate: "9".to_owned(),
        display_name: "Dirt Dynamos".to_owned(),
        is_team: true,
        is_dnf: false,
        riders: Vec::new(),
    };

    // Act
    let dnf = fixture.engine.entry_is_dnf(&riderless);

    // Assert
    assert_eq!(dnf, false);
}

// --------------------------------------------------- duplicate crossings

#[test]
fn test_duplicate_crossings_given_no_crossings_is_empty() {
    // Arrange
    let mut fixture = running_fixture();
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));

    // Act
    let duplicates = fixture.engine.duplicate_crossings();

    // Assert
    assert_eq!(duplicates, Vec::new());
}

#[test]
fn test_duplicate_crossings_given_one_repeated_instant_returns_one_pair() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(two_entries()));
    let first = record_at(&mut fixture, "12", at(10, 1, 0));
    let second = record_at(&mut fixture, "12", at(10, 1, 0));

    // Act
    let duplicates = fixture.engine.duplicate_crossings();

    // Assert
    assert_eq!(duplicates.len(), 1);
    assert_eq!(
        duplicates
            .first()
            .map(|(older, newer)| (older.seq, newer.seq)),
        Some((first.lap, second.lap))
    );
}

#[test]
fn test_duplicate_crossings_given_two_entries_at_one_instant_is_empty() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(two_entries()));
    let _ = record_at(&mut fixture, "12", at(10, 1, 0));
    let _ = record_at(&mut fixture, "34", at(10, 1, 0));

    // Act
    let duplicates = fixture.engine.duplicate_crossings();

    // Assert
    assert_eq!(duplicates, Vec::new());
}

#[test]
fn test_duplicate_crossings_given_laps_one_second_apart_is_empty() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(two_entries()));
    let _ = record_at(&mut fixture, "12", at(10, 1, 0));
    let _ = record_at(&mut fixture, "12", at(10, 1, 1));

    // Act
    let duplicates = fixture.engine.duplicate_crossings();

    // Assert
    assert_eq!(duplicates, Vec::new());
}

#[test]
fn test_duplicate_crossings_given_three_at_one_instant_returns_three_pairs() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(two_entries()));
    let _ = record_at(&mut fixture, "12", at(10, 1, 0));
    let _ = record_at(&mut fixture, "12", at(10, 1, 0));
    let _ = record_at(&mut fixture, "12", at(10, 1, 0));

    // Act
    let duplicates = fixture.engine.duplicate_crossings();

    // Assert
    assert_eq!(
        duplicates
            .iter()
            .map(|(older, newer)| (older.seq, newer.seq))
            .collect::<Vec<_>>(),
        vec![(1, 2), (1, 3), (2, 3)]
    );
}

#[test]
fn test_duplicate_crossings_are_ordered_oldest_instant_first() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(two_entries()));
    let _ = record_at(&mut fixture, "34", at(10, 5, 0));
    let _ = record_at(&mut fixture, "34", at(10, 5, 0));
    let _ = record_at(&mut fixture, "12", at(10, 1, 0));
    let _ = record_at(&mut fixture, "12", at(10, 1, 0));

    // Act
    let duplicates = fixture.engine.duplicate_crossings();

    // Assert
    assert_eq!(
        duplicates
            .iter()
            .map(|(older, _newer)| older.crossed_at)
            .collect::<Vec<_>>(),
        vec![at(10, 1, 0), at(10, 5, 0)]
    );
}

// ------------------------------------------------------- finish tie-break

#[test]
fn test_finish_given_two_equal_hands_draws_one_card_each() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture),
        vec![
            ("12".to_owned(), Some(card("5H"))),
            ("34".to_owned(), Some(card("AH"))),
        ]
    );
}

#[test]
fn test_finish_given_two_equal_hands_appends_the_tiebreak_draw_event() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let key_34 = key(&fixture.roster, "34");

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        fixture.engine.events().last(),
        Some(&Event {
            action: EventAction::TiebreakDraw,
            payload: json!({
                "draws": [
                    {"entry_id": key_12.0, "card": "5H"},
                    {"entry_id": key_34.0, "card": "AH"},
                ],
                "summary": "12 · 5H, 34 · AH",
            }),
        })
    );
}

#[test]
fn test_finish_returns_the_finish_event_before_the_draw_event() {
    // Arrange
    let mut fixture = running_fixture();

    // Act
    let event = fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(event.action, EventAction::Finish);
    assert_eq!(
        fixture
            .engine
            .events()
            .iter()
            .rev()
            .take(2)
            .map(|event| event.action)
            .collect::<Vec<_>>(),
        vec![EventAction::TiebreakDraw, EventAction::Finish]
    );
}

#[test]
fn test_finish_given_distinct_hands_appends_no_tiebreak_draw() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(two_entries()));
    let _ = record_at(&mut fixture, "12", at(10, 1, 0));

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        fixture.engine.events().last().map(|event| event.action),
        Some(EventAction::Finish)
    );
    assert_eq!(
        draws_by_plate(&fixture),
        vec![("12".to_owned(), None), ("34".to_owned(), None)]
    );
}

#[test]
fn test_finish_given_one_tied_pair_and_one_odd_hand_draws_the_pair_only() {
    // Arrange
    let mut fixture = started_with(
        always_deal_config(),
        FakeRoster::new(vec![solo_entry("12"), solo_entry("34"), solo_entry("56")]),
    );
    let _ = record_at(&mut fixture, "56", at(10, 1, 0));

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture),
        vec![
            ("12".to_owned(), Some(card("5H"))),
            ("34".to_owned(), Some(card("AH"))),
            ("56".to_owned(), None),
        ]
    );
}

#[test]
fn test_finish_given_a_dnf_entry_leaves_it_out_of_the_draw() {
    // Arrange
    let mut fixture = started_with(
        always_deal_config(),
        FakeRoster::new(vec![solo_entry("12"), solo_entry("34"), solo_entry("56")]),
    );
    fixture
        .engine
        .mark_dnf("56", "withdrew")
        .expect("the ride is running");

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture),
        vec![
            ("12".to_owned(), Some(card("5H"))),
            ("34".to_owned(), Some(card("AH"))),
            ("56".to_owned(), None),
        ]
    );
}

#[test]
fn test_finish_given_a_lone_entry_appends_no_tiebreak_draw() {
    // Arrange
    let mut fixture = started_with(fixture_config(), FakeRoster::new(vec![solo_entry("12")]));

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        fixture.engine.events().last().map(|event| event.action),
        Some(EventAction::Finish)
    );
}

#[test]
fn test_finish_given_two_tie_groups_hands_out_one_deck_in_order() {
    // Arrange
    let config = config_with(RideConfigParams {
        deck_count: 1,
        jokers_per_deck: 0,
        min_lap_s: 1,
        ..params()
    });
    let mut fixture = fixture_with_seed(
        config,
        FakeRoster::new(vec![
            solo_entry("12"),
            solo_entry("34"),
            solo_entry("56"),
            solo_entry("78"),
        ]),
        20_260_004,
    );
    fixture.engine.start(None).expect("the roster is ready");
    let _ = record_at(&mut fixture, "12", at(10, 0, 1));
    let _ = record_at(&mut fixture, "34", at(10, 0, 2));

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture),
        vec![
            ("12".to_owned(), Some(card("2H"))),
            ("34".to_owned(), Some(card("2C"))),
            ("56".to_owned(), Some(card("5H"))),
            ("78".to_owned(), Some(card("JS"))),
        ]
    );
}

#[test]
fn test_finish_given_fifty_two_tied_entries_draws_the_whole_deck() {
    // Arrange
    let mut fixture = started_with(fixture_config(), FakeRoster::new(tied_field(52)));

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        fixture
            .engine
            .snapshot()
            .iter()
            .filter(|entry| entry.tiebreak_card.is_some())
            .count(),
        52
    );
}

#[test]
fn test_finish_given_fifty_three_tied_entries_leaves_the_extra_undrawn() {
    // Arrange
    let mut fixture = started_with(fixture_config(), FakeRoster::new(tied_field(53)));

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        fixture
            .engine
            .snapshot()
            .iter()
            .filter(|entry| entry.tiebreak_card.is_some())
            .count(),
        52
    );
    assert_eq!(
        fixture
            .engine
            .snapshot()
            .iter()
            .find(|entry| entry.plate == "53")
            .map(|entry| entry.tiebreak_card.clone()),
        Some(None)
    );
}

#[test]
fn test_finish_given_the_same_stored_seed_draws_the_same_cards() {
    // Arrange
    let mut first = running_fixture();
    first.engine.finish(&[]).expect("the ride is running");
    let mut second = running_fixture();

    // Act
    second.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(draws_by_plate(&second), draws_by_plate(&first));
}

#[test]
fn test_finish_given_a_tie_draws_without_moving_the_shoe() {
    // Arrange
    let mut fixture = running_fixture();
    let remaining_before = fixture.engine.shoe_remaining();

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(fixture.engine.shoe_remaining(), remaining_before);
}

#[test]
fn test_finish_draws_with_the_seed_salted_by_the_xor_constant() {
    // Arrange
    let mut fixture = running_fixture();
    let expected = high_card_draw(SEED ^ TIEBREAK_XOR, 2);

    // Act
    fixture.engine.finish(&[]).expect("the ride is running");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture)
            .iter()
            .map(|(_plate, card)| card.clone())
            .collect::<Vec<_>>(),
        expected.iter().cloned().map(Some).collect::<Vec<_>>()
    );
}

#[test]
fn test_reopen_given_drawn_cards_clears_the_tiebreak() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.engine.finish(&[]).expect("the ride is running");

    // Act
    fixture.engine.reopen().expect("the ride is finished");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture),
        vec![("12".to_owned(), None), ("34".to_owned(), None)]
    );
}

#[test]
fn test_start_given_a_reopened_ride_clears_the_recorded_draws() {
    // Arrange
    let mut fixture = reopened_fixture();
    let key_12 = key(&fixture.roster, "12");
    fixture
        .engine
        .apply(&Event {
            action: EventAction::TiebreakDraw,
            payload: json!({
                "draws": [{"entry_id": key_12.0.clone(), "card": "AS"}],
                "summary": "12 · AS",
            }),
        })
        .expect("the row replays");

    // Act
    fixture.engine.start(None).expect("the ride continues");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture),
        vec![("12".to_owned(), None), ("34".to_owned(), None)]
    );
}

// ---------------------------------------------------- replay equivalence

#[test]
fn test_replay_of_a_lifecycle_ride_reproduces_the_observable_state() {
    // Arrange
    let mut live = running_fixture();
    let _ = record_at(&mut live, "12", at(10, 30, 0));
    let _ = record_at(&mut live, "12", at(11, 0, 0));
    live.engine.stop().expect("the ride is running");
    live.engine.start(None).expect("the ride continues");
    let _ = record_at(&mut live, "34", at(11, 30, 0));
    live.engine.finish(&[]).expect("the ride is running");
    live.engine.reopen().expect("the ride is finished");
    live.engine
        .deal_manual("12", Some("reopened bonus"))
        .expect("the shoe is open");
    let config = live.engine.config().clone();
    let entries = live.roster.entries();
    let events = live.engine.events().to_vec();

    // Act
    let replayed = replay_fixture(config, entries, &events);

    // Assert
    assert_eq!(observe_state(&replayed), observe_state(&live));
    assert_eq!(replayed.engine.events().to_vec(), events);
}

#[test]
fn test_replay_of_a_held_ride_reproduces_the_observable_state() {
    // Arrange
    let mut live = running_fixture();
    let _ = record_at(&mut live, "12", at(10, 0, 30));
    let held = live
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");
    live.engine
        .confirm_held(&held.crossing)
        .expect("the card is held");
    let _ = record_at(&mut live, "12", at(10, 31, 0));
    let credited = last_crossing(&live);
    live.engine
        .return_to_held(&credited)
        .expect("the card is credited");
    live.engine
        .confirm_held(&credited)
        .expect("the card is held again");
    let _ = record_at(&mut live, "12", at(10, 32, 0));
    let short = last_crossing(&live);
    live.engine
        .void_held(&short)
        .expect("the short card is held");
    let config = live.engine.config().clone();
    let entries = live.roster.entries();
    let events = live.engine.events().to_vec();

    // Act
    let replayed = replay_fixture(config, entries, &events);

    // Assert
    assert_eq!(observe_state(&replayed), observe_state(&live));
    assert_eq!(replayed.engine.events().to_vec(), events);
}

#[test]
fn test_replay_of_a_corrections_ride_reproduces_the_observable_state() {
    // Arrange
    let mut live = running_fixture();
    let key_12 = key(&live.roster, "12");
    let _ = record_at(&mut live, "12", at(10, 30, 0));
    let _ = record_at(&mut live, "12", at(11, 0, 0));
    live.engine
        .deal_manual("34", Some("bonus card"))
        .expect("the ride is running");
    live.engine
        .edit_crossing(&key_12, 1, at(10, 20, 0), "clock drift")
        .expect("the lap is live");
    live.engine
        .add_crossing_at("34", at(12, 0, 0), "missed at the line")
        .expect("the ride is running");
    live.engine
        .record_miss(at(12, 5, 0), "missed number")
        .expect("the ride is running");
    live.engine
        .assign_plate_to_miss(1, "34", "rider identified")
        .expect("the miss is pending");
    live.engine
        .reassign_crossing(1, "34", "wrong plate at the line")
        .expect("the crossing is live");
    live.engine
        .mark_dnf("12", "withdrew")
        .expect("the ride is running");
    let card_12 = live
        .engine
        .credited_cards(&key_12)
        .first()
        .cloned()
        .expect("the entry holds a credited card");
    live.engine
        .void_card("12", &card_12, "wrong card dealt")
        .expect("the card is credited");
    live.engine.undo_last().expect("a crossing is live");
    let config = live.engine.config().clone();
    let entries = live.roster.entries();
    let events = live.engine.events().to_vec();

    // Act
    let replayed = replay_fixture(config, entries, &events);

    // Assert
    assert_eq!(observe_state(&replayed), observe_state(&live));
    assert_eq!(replayed.engine.events().to_vec(), events);
}

#[test]
fn test_replay_of_a_move_ride_reproduces_the_observable_state() {
    // Arrange
    let mut live = running_entries(two_team_pooled());
    let beta = key(&live.roster, "3");
    let _ = record_at(&mut live, "2", at(10, 30, 0));
    live.engine.stop().expect("the ride is running");
    live.engine
        .move_rider("2", "Team B", "rider swapped teams")
        .expect("the ride is stopped");
    let config = live.engine.config().clone();
    let entries = live.roster.entries();
    let events = live.engine.events().to_vec();
    let moved_beta = beta;

    // Act
    let replayed = replay_fixture(config, entries, &events);

    // Assert
    assert_eq!(observe_state(&replayed), observe_state(&live));
    assert_eq!(replayed.engine.credited_cards(&moved_beta).len(), 1);
    assert_eq!(
        replayed.engine.events().last().map(|event| event.action),
        Some(EventAction::MoveRider)
    );
}

#[test]
fn test_replay_of_a_reshuffle_ride_reproduces_the_observable_state() {
    // Arrange
    let config = config_with(RideConfigParams {
        deck_count: 1,
        jokers_per_deck: 0,
        min_lap_s: 1,
        hold_short_laps: false,
        ..params()
    });
    let mut live = started_with(config, FakeRoster::new(vec![solo_entry("12")]));
    record_many(&mut live.engine, "12", 53, at(10, 1, 0), 60);
    assert_eq!(
        live.engine
            .events()
            .iter()
            .filter(|event| event.action == EventAction::ShoeReshuffle)
            .count(),
        1
    );
    let config = live.engine.config().clone();
    let entries = live.roster.entries();
    let events = live.engine.events().to_vec();

    // Act
    let replayed = replay_fixture(config, entries, &events);

    // Assert
    assert_eq!(observe_state(&replayed), observe_state(&live));
    assert_eq!(replayed.engine.events().to_vec(), events);
}

#[test]
fn test_replay_of_a_finished_tied_ride_reproduces_the_draw_and_snapshot() {
    // Arrange
    let mut live = running_fixture();
    live.engine.finish(&[]).expect("the ride is running");
    let config = live.engine.config().clone();
    let entries = live.roster.entries();
    let events = live.engine.events().to_vec();

    // Act
    let replayed = replay_fixture(config, entries, &events);

    // Assert
    assert_eq!(observe_state(&replayed), observe_state(&live));
    assert_eq!(replayed.engine.events().to_vec(), events);
    assert_eq!(
        draws_by_plate(&replayed),
        vec![
            ("12".to_owned(), Some(card("5H"))),
            ("34".to_owned(), Some(card("AH"))),
        ]
    );
}

// ------------------------------------- apply: further branch coverage

#[test]
fn test_apply_action_with_a_known_action_dispatches() {
    // Arrange
    let mut fixture = default_fixture();
    let key_12 = key(&fixture.roster, "12");

    // Act
    let result = fixture.engine.apply_action(
        "tiebreak_draw",
        &json!({
            "draws": [{"entry_id": key_12.0.clone(), "card": "AS"}],
            "summary": "12 · AS",
        }),
    );

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(
        draws_by_plate(&fixture),
        vec![("12".to_owned(), Some(card("AS"))), ("34".to_owned(), None)]
    );
}

#[test]
fn test_apply_finish_event_without_a_start_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    let event = Event {
        action: EventAction::Finish,
        payload: json!({"finished_at": "2026-09-20T10:30:00", "reason": "0:00:00"}),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
}

#[test]
fn test_apply_reopen_event_without_a_start_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    fixture.engine.state = RideStatus::Finished;
    let event = Event {
        action: EventAction::Reopen,
        payload: json!({"reopened_at": "2026-09-20T10:40:00", "reason": "0:00:00"}),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Err(illegal("ride has not started")));
}

#[test]
fn test_apply_record_crossing_on_a_closed_shoe_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.engine.finish(&[]).expect("the ride is running");
    let key_12 = key(&fixture.roster, "12");
    let event = Event {
        action: EventAction::RecordCrossing,
        payload: json!({
            "plate": "12",
            "entry_id": key_12.0.clone(),
            "lap": 1,
            "crossed_at": "2026-09-20T10:02:00",
            "reason": "Rider 12 · solo",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Err(illegal("shoe is closed")));
}

#[test]
fn test_apply_deal_manual_on_a_closed_shoe_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.engine.finish(&[]).expect("the ride is running");
    let key_12 = key(&fixture.roster, "12");
    let event = Event {
        action: EventAction::DealManual,
        payload: json!({
            "plate": "12",
            "entry_id": key_12.0.clone(),
            "card": "AS",
            "reason": "bonus",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Err(illegal("shoe is closed")));
}

#[test]
fn test_apply_add_crossing_at_before_the_latest_lap_is_refused() {
    // Arrange
    let mut fixture = started_with(always_deal_config(), FakeRoster::new(two_entries()));
    let key_12 = key(&fixture.roster, "12");
    let _ = record_at(&mut fixture, "12", at(10, 30, 0));
    let event = Event {
        action: EventAction::AddCrossingAt,
        payload: json!({
            "plate": "12",
            "entry_id": key_12.0.clone(),
            "crossed_at": "2026-09-20T10:20:00",
            "reason": "missed at the line",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(
        result,
        Err(RideError::LapTimeNotPositive {
            entry_label: "12".to_owned(),
            crossed_at: "2026-09-20T10:20:00".to_owned(),
            previous: "2026-09-20T10:30:00".to_owned(),
        })
    );
}

#[test]
fn test_apply_reassign_with_an_unknown_ordinal_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let key_34 = key(&fixture.roster, "34");
    let event = Event {
        action: EventAction::Reassign,
        payload: json!({
            "seq": 99,
            "new_entry_id": key_34.0.clone(),
            "new_plate": "34",
            "reason": "wrong plate at the line",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Err(illegal("no crossing at ordinal 99")));
}

#[test]
fn test_apply_void_card_for_an_uncredited_card_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let event = Event {
        action: EventAction::VoidCard,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "card": "AS",
            "reason": "wrong card dealt",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Err(illegal("no dealt card AS credited to 12")));
}

#[test]
fn test_apply_void_card_for_a_held_card_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 0, 30)));
    let held = fixture
        .engine
        .held_crossings()
        .first()
        .cloned()
        .expect("the short lap holds its card");
    let event = Event {
        action: EventAction::VoidCard,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "card": held.card.code(),
            "reason": "wrong card dealt",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(
        result,
        Err(illegal(
            "card is held; confirm or void it through the review panel"
        ))
    );
}

#[test]
fn test_apply_with_a_missing_payload_key_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::Finish,
        payload: json!({"reason": "0:00:00"}),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Err(illegal("payload has no finished_at")));
}

#[test]
fn test_apply_with_an_invalid_timestamp_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let event = Event {
        action: EventAction::SetStartTime,
        payload: json!({"actual_start": "not-a-time"}),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Err(illegal("invalid actual_start: not-a-time")));
}

#[test]
fn test_apply_with_a_numeric_string_seq_replays() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let _ = fixture.engine.record_crossing("12", Some(at(10, 30, 0)));
    let event = Event {
        action: EventAction::VoidCrossing,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": "1",
            "reason": "double entry",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(fixture.engine.crossings().len(), 0);
}

#[test]
fn test_apply_with_an_out_of_range_seq_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let event = Event {
        action: EventAction::VoidCrossing,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": 4_294_967_296_u64,
            "reason": "double entry",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Err(illegal("seq is out of range")));
}

#[test]
fn test_apply_with_a_non_numeric_seq_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let event = Event {
        action: EventAction::VoidCrossing,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": true,
            "reason": "double entry",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Err(illegal("invalid seq")));
}

#[test]
fn test_apply_tiebreak_draw_with_a_bad_card_code_is_refused() {
    // Arrange
    let mut fixture = default_fixture();
    let event = Event {
        action: EventAction::TiebreakDraw,
        payload: json!({"draws": [{"entry_id": "key-12", "card": "ZZ"}]}),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Err(illegal("ZZ is not a valid card code")));
}

#[test]
fn test_apply_tiebreak_draw_with_a_malformed_row_is_skipped() {
    // Arrange
    let mut fixture = default_fixture();
    let event = Event {
        action: EventAction::TiebreakDraw,
        payload: json!({"draws": [{"card": "AS"}, {"entry_id": "key-12"}, 7]}),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(
        draws_by_plate(&fixture),
        vec![("12".to_owned(), None), ("34".to_owned(), None)]
    );
}

#[test]
fn test_apply_void_held_for_an_unrecorded_crossing_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let event = Event {
        action: EventAction::VoidHeld,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": 99,
            "card": "AS",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(
        result,
        Err(RideError::NoCrossing {
            entry_id: key_12.0,
            seq: 99,
        })
    );
}

#[test]
fn test_apply_return_to_held_for_an_unrecorded_crossing_is_refused() {
    // Arrange
    let mut fixture = running_fixture();
    let key_12 = key(&fixture.roster, "12");
    let event = Event {
        action: EventAction::ReturnToHeld,
        payload: json!({
            "entry_id": key_12.0.clone(),
            "seq": 99,
            "card": "AS",
        }),
    };

    // Act
    let result = fixture.engine.apply(&event);

    // Assert
    assert_eq!(
        result,
        Err(RideError::NoCrossing {
            entry_id: key_12.0,
            seq: 99,
        })
    );
}

#[test]
fn test_snapshot_given_a_solo_entry_with_no_rider_leaves_sex_none() {
    // Arrange
    let riderless = EntrySnapshot {
        id: EntryId("key-riderless".to_owned()),
        plate: "12".to_owned(),
        display_name: "Rider 12".to_owned(),
        is_team: false,
        is_dnf: false,
        riders: Vec::new(),
    };
    let fixture = fixture(fixture_config(), FakeRoster::new(vec![riderless]));

    // Act
    let results = fixture.engine.snapshot();

    // Assert
    assert_eq!(
        results.first().map(|entry| (entry.kind, entry.sex.clone())),
        Some((SnapshotKind::Solo, None))
    );
}

#[test]
fn test_entry_is_dnf_given_a_relay_team_is_false() {
    // Arrange
    let fixture = fixture(fixture_config(), FakeRoster::new(vec![relay_team()]));
    let relay = fixture
        .roster
        .resolve_plate("77")
        .expect("the relay team resolves");

    // Act
    let dnf = fixture.engine.entry_is_dnf(&relay);

    // Assert
    assert_eq!(dnf, false);
}

#[test]
fn test_finish_after_reopen_redraws_the_same_tiebreak() {
    // Arrange
    let mut fixture = running_fixture();
    fixture.engine.finish(&[]).expect("the ride is running");
    fixture.engine.reopen().expect("the ride is finished");

    // Act
    fixture.engine.finish(&[]).expect("the ride is reopened");

    // Assert
    assert_eq!(
        draws_by_plate(&fixture),
        vec![
            ("12".to_owned(), Some(card("5H"))),
            ("34".to_owned(), Some(card("AH"))),
        ]
    );
}
