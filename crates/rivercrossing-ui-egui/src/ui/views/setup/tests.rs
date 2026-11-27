// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the ride-setup form: the three-column format grid, the lock notice, the logo
//! row, the ISO date field and the tie-break chips' drag, click and keyboard reorder paths.

use std::cell::RefCell;
use std::rc::Rc;

use craft::egui::{Key, Modifiers, Rect, Vec2};
use craft::theme::Srgba;
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::runtime::env::Env;
use craft::ui::testing::Fixture;
use craft::ui::testing::focus::AccessActionKind;
use rivercrossing_cards::JokersMode;
use rivercrossing_ride::{EntryMode, PlateModel};

use super::{body, earlier, reordered, slot_at};
use crate::ui::state::{AppState, SetupRadios};

/// Return a blank state with the ride-setup dialog open, so its fields can be seeded.
fn setup_state() -> Rc<RefCell<AppState>> {
    let state = Rc::new(RefCell::new(AppState::blank()));
    let opened = state.borrow_mut().dialogs.open("ride_setup_dlg", None);

    // The frozen dialog accepts its own id.
    assert_eq!(opened, Ok(()));
    state
}

/// Return a fixture showing the ride-setup body over *state* in a wide room.
fn setup_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        body(ui, &mut shown.borrow_mut());
    });
    fixture.room(Vec2::new(900.0, 1400.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(2);
    fixture
}

/// Return the painted texts of the latest frame.
fn texts(fixture: &Fixture) -> Vec<&str> {
    fixture
        .texts()
        .iter()
        .map(|run| run.text.as_str())
        .collect()
}

/// Return whether the latest frame painted *needle* anywhere.
fn paints(fixture: &Fixture, needle: &str) -> bool {
    fixture.texts().iter().any(|run| run.text.contains(needle))
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

/// Return whether two rects share a vertical band.
fn same_row(left: Rect, right: Rect) -> bool {
    left.min.y < right.max.y && right.min.y < left.max.y
}

// ------------------------------------------------------------------ the notice

#[test]
fn test_body_shows_the_lock_notice() {
    // Arrange
    let state = setup_state();

    // Act
    let fixture = setup_fixture(&state);

    // Assert
    assert!(
        paints(&fixture, "lock once the ride starts"),
        "the mockup's lock notice shows"
    );
}

#[test]
fn test_body_puts_the_lock_notice_above_the_fields() {
    // Arrange
    let state = setup_state();

    // Act
    let fixture = setup_fixture(&state);
    let notice = text_rect(&fixture, "Ride name");
    let lock = fixture
        .texts()
        .iter()
        .find(|run| run.text.contains("lock once the ride starts"))
        .map(|run| run.rect)
        .expect("the lock notice shows");

    // Assert
    assert!(
        lock.max.y <= notice.min.y,
        "the notice comes first: {lock:?} {notice:?}"
    );
}

#[test]
fn test_body_keeps_the_error_notice() {
    // Arrange: the presenter refused the last save.
    let state = setup_state();
    state.borrow_mut().setup_errors = Some(vec!["Ride name is required".to_owned()]);

    // Act
    let fixture = setup_fixture(&state);

    // Assert
    assert!(paints(&fixture, "Ride name is required"));
}

// ------------------------------------------------------------------- the format

#[test]
fn test_body_places_team_size_and_plate_model_beside_entry_mode() {
    // Arrange
    let state = setup_state();

    // Act
    let fixture = setup_fixture(&state);
    let entry = text_rect(&fixture, "Entry mode");
    let team = text_rect(&fixture, "Team size");
    let plate = text_rect(&fixture, "Plate model");

    // Assert: the mockup's `.grid3` first row.
    assert!(same_row(entry, team), "Team size shares Entry mode's row");
    assert!(
        same_row(entry, plate),
        "Plate model shares Entry mode's row"
    );
}

#[test]
fn test_body_places_decks_jokers_and_jokers_counted_on_one_row() {
    // Arrange
    let state = setup_state();

    // Act
    let fixture = setup_fixture(&state);
    let entry = text_rect(&fixture, "Entry mode");
    let decks = text_rect(&fixture, "Decks");
    let jokers = text_rect(&fixture, "Jokers");
    let counted = text_rect(&fixture, "Jokers counted");

    // Assert: the grid's second row, under the first.
    assert!(same_row(decks, jokers), "Decks and Jokers share a row");
    assert!(same_row(decks, counted), "Jokers counted shares their row");
    assert!(
        decks.min.y >= entry.max.y,
        "the second row sits under the first"
    );
}

#[test]
fn test_body_names_the_card_group_cards() {
    // Arrange
    let state = setup_state();

    // Act
    let fixture = setup_fixture(&state);
    let shown = texts(&fixture);

    // Assert: neither the mockup nor §15b has "Shoe".
    assert!(shown.contains(&"Cards"), "the group is titled Cards");
    assert!(!shown.contains(&"Shoe"), "the group is not titled Shoe");
}

// --------------------------------------------------------------------- the logo

#[test]
fn test_body_logo_status_defaults_to_no_logo() {
    // Arrange: no logo chosen.
    let state = setup_state();

    // Act
    let fixture = setup_fixture(&state);
    let shown = texts(&fixture);

    // Assert: §15b's default word.
    assert!(shown.contains(&"NO LOGO"), "the default status is NO LOGO");
    assert!(
        !paints(&fixture, "No logo chosen"),
        "the old wording is gone"
    );
}

#[test]
fn test_body_logo_status_reads_ready_for_a_chosen_logo() {
    // Arrange
    let state = setup_state();
    state.borrow_mut().setup_logo = "logo.png".to_owned();

    // Act
    let fixture = setup_fixture(&state);
    let shown = texts(&fixture);

    // Assert
    assert!(shown.contains(&"Ready"), "a chosen logo reads ready");
    assert!(
        shown.contains(&"logo.png"),
        "the chip names the chosen file"
    );
}

// ------------------------------------------------------------------- the radios

/// Open the setup form over *seed*'s radio choices, click *label*, and return the state.
fn click_radio(seed: SetupRadios, label: &str) -> Rc<RefCell<AppState>> {
    let state = setup_state();
    state.borrow_mut().setup_radios = seed;
    let mut fixture = setup_fixture(&state);
    let target = fixture.by_label(label).expect("the radio shows");

    fixture.click(&target);
    state
}

#[test]
fn test_body_clicking_mixed_switches_the_entry_mode_to_mixed() {
    // Arrange, Act
    let state = click_radio(SetupRadios::default(), "Mixed (teams)");

    // Assert
    assert_eq!(state.borrow().setup_radios.entry_mode, EntryMode::Mixed);
}

#[test]
fn test_body_clicking_solo_switches_the_entry_mode_back_to_solo() {
    // Arrange
    let seed = SetupRadios {
        entry_mode: EntryMode::Mixed,
        ..SetupRadios::default()
    };

    // Act
    let state = click_radio(seed, "Solo");

    // Assert
    assert_eq!(state.borrow().setup_radios.entry_mode, EntryMode::Solo);
}

#[test]
fn test_body_clicking_team_relay_switches_the_plate_model() {
    // Arrange, Act
    let state = click_radio(SetupRadios::default(), "Team relay");

    // Assert
    assert_eq!(
        state.borrow().setup_radios.plate_model,
        PlateModel::TeamRelay
    );
}

#[test]
fn test_body_clicking_pooled_switches_the_plate_model_back() {
    // Arrange
    let seed = SetupRadios {
        plate_model: PlateModel::TeamRelay,
        ..SetupRadios::default()
    };

    // Act
    let state = click_radio(seed, "Rider plates (pooled)");

    // Assert
    assert_eq!(
        state.borrow().setup_radios.plate_model,
        PlateModel::RiderPooled
    );
}

#[test]
fn test_body_clicking_per_deck_switches_the_jokers_mode() {
    // Arrange, Act: the design's default counts jokers across the ride.
    let state = click_radio(SetupRadios::default(), "Per deck");

    // Assert
    assert_eq!(state.borrow().setup_radios.jokers_mode, JokersMode::PerDeck);
}

#[test]
fn test_body_clicking_total_switches_the_jokers_mode_back() {
    // Arrange
    let seed = SetupRadios {
        jokers_mode: JokersMode::PerDeck,
        ..SetupRadios::default()
    };

    // Act
    let state = click_radio(seed, "Total");

    // Assert
    assert_eq!(state.borrow().setup_radios.jokers_mode, JokersMode::Total);
}

#[test]
fn test_body_clicking_always_deal_releases_the_short_lap_cards() {
    // Arrange, Act
    let state = click_radio(SetupRadios::default(), "Always deal cards");

    // Assert
    assert!(!state.borrow().setup_radios.hold_short_laps);
}

#[test]
fn test_body_clicking_hold_short_laps_restores_the_hold() {
    // Arrange
    let seed = SetupRadios {
        hold_short_laps: false,
        ..SetupRadios::default()
    };

    // Act
    let state = click_radio(seed, "Hold short-lap cards for review");

    // Assert
    assert!(state.borrow().setup_radios.hold_short_laps);
}

#[test]
fn test_body_browse_button_raises_the_picker() {
    // Arrange
    let state = setup_state();
    let mut fixture = setup_fixture(&state);
    let browse = fixture.by_label("Browse…").expect("the Browse button");

    // Act
    fixture.click(&browse);

    // Assert: the app raises the platform picker on its next logic pass.
    assert!(state.borrow().choose_logo);
}

// --------------------------------------------------------------------- the date

#[test]
fn test_body_date_field_shows_the_seeded_iso_date() {
    // Arrange
    let state = setup_state();
    let seeded = state
        .borrow_mut()
        .dialogs
        .set_field("date_picker", "2026-06-13");
    assert_eq!(seeded, Ok(()));

    // Act
    let fixture = setup_fixture(&state);
    let shown = texts(&fixture);

    // Assert: the mockup writes the date as `2026-06-13`.
    assert!(shown.contains(&"2026-06-13"), "the date field shows ISO");
}

// ---------------------------------------------------------------- tie-break chips

#[test]
fn test_body_chips_size_to_their_labels() {
    // Arrange
    let state = setup_state();

    // Act
    let fixture = setup_fixture(&state);
    let best = fixture.by_label("Best hand").expect("the first chip");
    let laps = fixture.by_label("Laps").expect("the second chip");

    // Assert: content width, not one fixed cell.
    assert!(
        best.rect.width() > laps.rect.width(),
        "the chip sizes to its label: {best:?} {laps:?}"
    );
    assert!(
        laps.rect.min.x >= best.rect.max.x,
        "the chips keep their gap: {best:?} {laps:?}"
    );
}

#[test]
fn test_body_clicking_a_chip_moves_it_one_place_earlier() {
    // Arrange
    let state = setup_state();
    let mut fixture = setup_fixture(&state);
    let chip = fixture.by_label("Best hand").expect("the first chip");

    // Act: a single-pointer path beside the drag.
    fixture.click(&chip);

    // Assert: the leading chip cycles to the end.
    assert_eq!(
        state.borrow().setup_tiebreak,
        ["laps", "total_time", "high_card"]
    );
}

#[test]
fn test_body_a_focused_chip_moves_earlier_on_enter() {
    // Arrange
    let state = setup_state();
    let mut fixture = setup_fixture(&state);
    let chip = fixture.by_label("Best hand").expect("the first chip");

    // Act: the keyboard path.
    fixture.access(AccessActionKind::Focus, &chip);
    fixture.key(Key::Enter, Modifiers::NONE);

    // Assert
    assert_eq!(
        state.borrow().setup_tiebreak,
        ["laps", "total_time", "high_card"]
    );
}

#[test]
fn test_body_a_focused_chip_moves_earlier_on_space() {
    // Arrange
    let state = setup_state();
    let mut fixture = setup_fixture(&state);
    let chip = fixture.by_label("Best hand").expect("the first chip");

    // Act: Space is the keyboard's other activation key.
    fixture.access(AccessActionKind::Focus, &chip);
    fixture.key(Key::Space, Modifiers::NONE);

    // Assert
    assert_eq!(
        state.borrow().setup_tiebreak,
        ["laps", "total_time", "high_card"]
    );
}

#[test]
fn test_body_enter_with_no_chip_focused_keeps_the_order() {
    // Arrange
    let state = setup_state();
    let mut fixture = setup_fixture(&state);

    // Act
    fixture.key(Key::Enter, Modifiers::NONE);

    // Assert
    assert_eq!(
        state.borrow().setup_tiebreak,
        ["high_card", "laps", "total_time"]
    );
}

#[test]
fn test_body_a_focused_chip_draws_the_focus_ring() {
    // Arrange
    let state = setup_state();
    let mut fixture = setup_fixture(&state);
    let chip = fixture.by_label("Best hand").expect("the first chip");
    let focus = craft::egui::Color32::from(Srgba::from(
        Env::of_root(fixture.ctx()).tokens.color.line.focus,
    ));
    fixture.access(AccessActionKind::Focus, &chip);

    // Act
    let shapes = fixture.frame_shapes();

    // Assert
    let ringed = shapes.iter().any(|shape| {
        matches!(shape, craft::egui::Shape::Rect(rect) if rect.stroke.color == focus && rect.stroke.width > 0.0)
    });
    assert!(ringed, "the focused chip shows its ring");
}

#[test]
fn test_body_dragging_a_chip_onto_the_next_place_moves_it_there() {
    // Arrange
    let state = setup_state();
    let mut fixture = setup_fixture(&state);
    let chip = fixture.by_label("Best hand").expect("the first chip");
    // A drop just past the first chip's end names the second slot.
    let by = Vec2::new(chip.rect.width() / 2.0 + 2.0, 0.0);

    // Act
    fixture.drag(&chip, by);

    // Assert
    assert_eq!(
        state.borrow().setup_tiebreak,
        ["laps", "high_card", "total_time"]
    );
}

#[test]
fn test_body_dragging_a_chip_onto_its_own_place_keeps_the_order() {
    // Arrange
    let state = setup_state();
    let mut fixture = setup_fixture(&state);
    let chip = fixture.by_label("Best hand").expect("the first chip");
    // A drop left of every chip names the first slot, where the chip already is.
    let by = Vec2::new(-chip.rect.width(), 0.0);

    // Act
    fixture.drag(&chip, by);

    // Assert
    assert_eq!(
        state.borrow().setup_tiebreak,
        ["high_card", "laps", "total_time"]
    );
}

// ---------------------------------------------------------------- the pure helpers

#[test]
fn test_earlier_given_the_first_chip_wraps_to_the_end() {
    // Act
    let at = earlier(0, 3);

    // Assert
    assert_eq!(at, 2);
}

#[test]
fn test_earlier_given_the_last_chip_moves_one_back() {
    // Act
    let at = earlier(2, 3);

    // Assert
    assert_eq!(at, 1);
}

#[test]
fn test_earlier_given_a_middle_chip_moves_one_back() {
    // Act
    let at = earlier(1, 3);

    // Assert
    assert_eq!(at, 0);
}

#[test]
fn test_reordered_moves_the_first_chip_to_the_end() {
    // Act
    let order = reordered(["a", "b", "c"], 0, 2);

    // Assert
    assert_eq!(order, ["b", "c", "a"]);
}

#[test]
fn test_reordered_moves_the_last_chip_to_the_front() {
    // Act
    let order = reordered(["a", "b", "c"], 2, 0);

    // Assert
    assert_eq!(order, ["c", "a", "b"]);
}

#[test]
fn test_reordered_clamps_a_slot_past_the_end() {
    // Act
    let order = reordered(["a", "b", "c"], 0, 9);

    // Assert
    assert_eq!(order, ["b", "c", "a"]);
}

#[test]
fn test_reordered_given_an_unknown_chip_keeps_the_order() {
    // Act
    let order = reordered(["a", "b", "c"], 3, 0);

    // Assert
    assert_eq!(order, ["a", "b", "c"]);
}

#[test]
fn test_slot_at_left_of_every_chip_names_the_first() {
    // Act
    let slot = slot_at(&[10.0, 40.0, 70.0], 0.0);

    // Assert
    assert_eq!(slot, 0);
}

#[test]
fn test_slot_at_on_a_chips_centre_names_that_chip() {
    // Act
    let slot = slot_at(&[10.0, 40.0, 70.0], 40.0);

    // Assert
    assert_eq!(slot, 1);
}

#[test]
fn test_slot_at_right_of_every_chip_names_the_last() {
    // Act
    let slot = slot_at(&[10.0, 40.0, 70.0], 90.0);

    // Assert
    assert_eq!(slot, 2);
}
