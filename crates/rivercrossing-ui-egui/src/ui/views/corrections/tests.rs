// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the Crossing Detail's aligned two-column value grid.

use std::cell::RefCell;
use std::rc::Rc;

use craft::egui::{Rect, Ui, Vec2};
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::testing::Fixture;
use rivercrossing_cards::Card;
use rivercrossing_ride::EntryId;

use super::{detail, dnf, edit_crossing, set_start};
use crate::ui::presenters::corrections::{CorrectionActions, CrossingDetailModel};
use crate::ui::state::{AppState, CorrectionTarget, CrossingDetailState};

/// Return the mockup's crossing.
fn model() -> CrossingDetailModel {
    CrossingDetailModel {
        rider: "Sam Ellis".to_owned(),
        team: "River Rats".to_owned(),
        plate: "123".to_owned(),
        lap: "4".to_owned(),
        time: "22:41".to_owned(),
        lap_time: "21:58".to_owned(),
        total: "1:31:12".to_owned(),
        card: "9♥".to_owned(),
        held: "No — credited".to_owned(),
    }
}

/// The four correction gates the fixture opens.
const ACTIONS: CorrectionActions = CorrectionActions {
    edit: true,
    edit_time: true,
    void_card: true,
    delete: true,
};

/// Return a state whose Crossing Detail is open on the mockup's crossing.
fn detail_state() -> Rc<RefCell<AppState>> {
    let state = Rc::new(RefCell::new(AppState::blank()));
    state.borrow_mut().crossing_detail = Some(CrossingDetailState {
        entry: EntryId("Sam Ellis".to_owned()),
        seq: 4,
        model: model(),
        actions: ACTIONS,
        card: None,
    });
    state
}

/// Return a fixture showing the Crossing Detail body over *state*.
fn detail_fixture(state: &Rc<RefCell<AppState>>) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        detail(ui, &mut shown.borrow_mut());
    });
    fixture.room(Vec2::new(760.0, 600.0));
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

/// Assert that each of *captions* starts at *origin*, within a pixel.
fn assert_column(fixture: &Fixture, origin: f32, captions: &[&str]) {
    for caption in captions {
        let left = text_rect(fixture, caption).min.x;
        assert!(
            (left - origin).abs() < 0.5,
            "{caption} starts at the column origin: {left} vs {origin}"
        );
    }
}

#[test]
fn test_detail_given_nine_values_aligns_the_right_column() {
    // Arrange
    let state = detail_state();

    // Act
    let fixture = detail_fixture(&state);
    let team = text_rect(&fixture, "Team").min.x;

    // Assert: every right-column caption starts at the same x, however wide its neighbour is.
    assert_column(&fixture, team, &["Lap", "Lap time", "Card"]);
}

#[test]
fn test_detail_given_nine_values_aligns_the_left_column() {
    // Arrange
    let state = detail_state();

    // Act
    let fixture = detail_fixture(&state);
    let rider = text_rect(&fixture, "Rider").min.x;

    // Assert
    assert_column(&fixture, rider, &["Plate", "Time", "Total", "Status"]);
}

/// Assert that each of *captions* sits one pane's width right of *origin*.
fn assert_second_column(fixture: &Fixture, origin: f32, pane: f32, captions: &[&str]) {
    for caption in captions {
        let left = text_rect(fixture, caption).min.x;
        assert!(
            ((left - origin) - pane).abs() < 0.5,
            "{caption} shares the column width: {} vs {pane}",
            left - origin
        );
    }
}

#[test]
fn test_detail_given_nine_values_gives_both_columns_the_same_width() {
    // Arrange
    let state = detail_state();

    // Act
    let fixture = detail_fixture(&state);
    let rider = text_rect(&fixture, "Rider").min.x;
    let team = text_rect(&fixture, "Team").min.x;

    // Assert: the two panes are equal, so the right column is centred between them.
    let pane = team - rider;
    assert!(
        pane > 1.0,
        "the right column stands clear of the left: {rider} {team}"
    );
    assert_second_column(&fixture, rider, pane, &["Lap", "Lap time", "Card"]);
}

// --------------------------------------------------- correction dialogs

/// Return a fixture showing *body* over *state*.
fn body_fixture(state: &Rc<RefCell<AppState>>, body: fn(&mut Ui, &mut AppState)) -> Fixture {
    let shown = Rc::clone(state);
    let mut fixture = Fixture::new(move |ui| {
        body(ui, &mut shown.borrow_mut());
    });
    fixture.room(Vec2::new(520.0, 600.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(3);
    fixture
}

/// Return a state whose Edit Crossing dialog is open on the mockup's crossing, with *card* held.
fn edit_state(add: bool, card: Option<Card>) -> Rc<RefCell<AppState>> {
    let state = Rc::new(RefCell::new(AppState::blank()));
    let mut app = state.borrow_mut();
    app.correction = CorrectionTarget {
        entry: Some(EntryId("sam".to_owned())),
        seq: Some(4),
        add,
    };
    app.crossing_detail = Some(CrossingDetailState {
        entry: EntryId("sam".to_owned()),
        seq: 4,
        model: model(),
        actions: ACTIONS,
        card,
    });
    drop(app);
    state
}

#[test]
fn test_edit_crossing_given_a_selected_crossing_shows_the_void_button() {
    // Arrange
    let state = edit_state(false, Some(Card::parse("9H").expect("the card code")));

    // Act
    let fixture = body_fixture(&state, edit_crossing);

    // Assert
    assert!(
        fixture.by_label("Void card…").is_some(),
        "the edit dialog carries its void action"
    );
}

#[test]
fn test_edit_crossing_given_the_void_button_clicked_opens_the_void_dialog() {
    // Arrange
    let state = edit_state(false, Some(Card::parse("9H").expect("the card code")));
    let mut fixture = body_fixture(&state, edit_crossing);
    let button = fixture.by_label("Void card…").expect("the void button");

    // Act
    fixture.click(&button);

    // Assert
    assert_eq!(
        state.borrow().dialogs.top().map(|dialog| dialog.id),
        Some("void_card_confirm_dlg")
    );
}

#[test]
fn test_edit_crossing_given_add_mode_hides_the_void_button() {
    // Arrange
    let state = edit_state(true, Some(Card::parse("9H").expect("the card code")));

    // Act
    let fixture = body_fixture(&state, edit_crossing);

    // Assert
    assert!(
        fixture.by_label("Void card…").is_none(),
        "Add Crossing offers no void action"
    );
}

#[test]
fn test_dnf_given_a_typed_entry_writes_it_to_the_entry_field() {
    // Arrange
    let state = Rc::new(RefCell::new(AppState::blank()));
    let mut fixture = body_fixture(&state, dnf);
    let field = fixture.by_label("Entry").expect("the Entry field");

    // Act
    fixture.click(&field);
    fixture.type_text("Alpha");

    // Assert: Entry is a typed field, not a read-only sentence.
    assert_eq!(state.borrow().correction_entry, "Alpha");
}

#[test]
fn test_set_start_given_an_empty_date_shows_no_epoch_date() {
    // Arrange
    let state = Rc::new(RefCell::new(AppState::blank()));
    state
        .borrow_mut()
        .dialogs
        .open("set_start_dlg", None)
        .expect("the start dialog opens");

    // Act
    let fixture = body_fixture(&state, set_start);

    // Assert
    let shows_epoch = fixture
        .texts()
        .iter()
        .any(|run| run.text == "1/1/70" || run.text == "1970-01-01");
    assert!(
        !shows_epoch,
        "an empty value shows no fabricated epoch date"
    );
}

#[test]
fn test_set_start_given_an_empty_date_shows_the_placeholder() {
    // Arrange
    let state = Rc::new(RefCell::new(AppState::blank()));
    state
        .borrow_mut()
        .dialogs
        .open("set_start_dlg", None)
        .expect("the start dialog opens");

    // Act
    let fixture = body_fixture(&state, set_start);

    // Assert
    assert!(
        fixture.texts().iter().any(|run| run.text == "YYYY-MM-DD"),
        "the empty date field offers its format as a placeholder"
    );
}

#[test]
fn test_edit_crossing_given_no_credited_card_hides_the_void_button() {
    // Arrange
    let state = edit_state(false, None);

    // Act
    let fixture = body_fixture(&state, edit_crossing);

    // Assert: there is no card to void.
    assert!(
        fixture.by_label("Void card…").is_none(),
        "a crossing with no card offers no void action"
    );
}
