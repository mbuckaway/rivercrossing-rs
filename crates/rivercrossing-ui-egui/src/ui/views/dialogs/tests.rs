// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the dialog host's button roles and close mapping.

use craft::egui::Vec2;
use craft::ui::contract::{LeastWidth, Sizing};
use craft::ui::fonts::{I18nScripts, UiFonts};
use craft::ui::runtime::access::Role;
use craft::ui::runtime::overlay::CloseReason;
use craft::ui::sections::DialogRole;
use craft::ui::testing::Fixture;

use super::{
    Activation, CONTROL_SIZING, activation, button_label, button_role, decimal_value, dialog_title,
    field_caption, least_width, message_body, message_spans, radio_group, select_value, triple,
};
use crate::ui::dialogs::spec::{Button, DialogSpec, spec};
use crate::ui::state::AppState;

/// Return the frozen contract of *id*; the test fixtures name frozen dialogs only.
fn dialog(id: &str) -> &'static DialogSpec {
    spec(id).expect("the fixture names a frozen dialog")
}

#[test]
fn test_button_role_given_the_enter_default_returns_accept() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let role = button_role(dialog, Button::Ok);

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_the_cancel_button_returns_cancel() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let role = button_role(dialog, Button::Cancel);

    // Assert
    assert_eq!(role, DialogRole::Cancel);
}

#[test]
fn test_button_role_given_the_delete_button_returns_destructive() {
    // Arrange
    let dialog = dialog("delete_ride_dlg");

    // Act
    let role = button_role(dialog, Button::Delete);

    // Assert
    assert_eq!(role, DialogRole::Destructive);
}

#[test]
fn test_button_role_given_a_destructive_dialogs_cancel_returns_accept() {
    // Arrange: Delete Ride defaults to Cancel, so Enter must accept the safe button.
    let dialog = dialog("delete_ride_dlg");

    // Act
    let role = button_role(dialog, Button::Cancel);

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_a_library_close_returns_cancel() {
    // Arrange: the ride library accepts on Open, so Close leaves without an outcome.
    let dialog = dialog("ride_library_dlg");

    // Act
    let role = button_role(dialog, Button::Close);

    // Assert
    assert_eq!(role, DialogRole::Cancel);
}

#[test]
fn test_button_role_given_the_quit_that_keeps_the_ride_returns_destructive() {
    // Arrange: Quit — keep ride running is neither the default nor the cancel.
    let dialog = dialog("exit_running_dlg");

    // Act
    let role = button_role(dialog, Button::Ok);

    // Assert
    assert_eq!(role, DialogRole::Destructive);
}

#[test]
fn test_button_role_given_the_exit_cancel_returns_accept() {
    // Arrange: Cancel is the safe default of Exit Running.
    let dialog = dialog("exit_running_dlg");

    // Act
    let role = button_role(dialog, Button::Cancel);

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_a_secondary_custom_button_returns_other() {
    // Arrange
    let dialog = dialog("resume_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("library_btn"));

    // Assert
    assert_eq!(role, DialogRole::Other);
}

#[test]
fn test_button_role_given_the_resume_default_returns_accept() {
    // Arrange
    let dialog = dialog("resume_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("continue_btn"));

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_the_simulator_go_button_returns_accept() {
    // Arrange
    let dialog = dialog("simulation_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("go_btn"));

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_the_crossing_delete_returns_destructive() {
    // Arrange
    let dialog = dialog("crossing_detail_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("delete_btn"));

    // Assert
    assert_eq!(role, DialogRole::Destructive);
}

#[test]
fn test_button_role_given_a_single_cancel_running_dialog_returns_accept() {
    // Arrange: the running dialogs' only button is both default and cancel.
    let dialog = dialog("sim_running_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("cancel_btn"));

    // Assert
    assert_eq!(role, DialogRole::Accept);
}

#[test]
fn test_button_role_given_the_crossing_void_card_returns_destructive() {
    // Arrange
    let dialog = dialog("crossing_detail_dlg");

    // Act
    let role = button_role(dialog, Button::Custom("void_card_btn"));

    // Assert
    assert_eq!(role, DialogRole::Destructive);
}

#[test]
fn test_dialog_title_given_a_new_setup_returns_ride_setup() {
    // Arrange
    let state = AppState::blank();

    // Act
    let title = dialog_title(&state, dialog("ride_setup_dlg"));

    // Assert
    assert_eq!(title, "Ride Setup");
}

#[test]
fn test_dialog_title_given_an_edit_setup_returns_edit_ride() {
    // Arrange
    let mut state = AppState::blank();
    state.setup_edit = Some(7);

    // Act
    let title = dialog_title(&state, dialog("ride_setup_dlg"));

    // Assert
    assert_eq!(title, "Edit Ride");
}

#[test]
fn test_least_width_given_a_table_dialog_returns_the_wide_role() {
    // Arrange
    let dialog = dialog("ride_library_dlg");

    // Act
    let least = least_width(dialog);

    // Assert
    assert_eq!(least, LeastWidth::WideDialog);
}

#[test]
fn test_least_width_given_a_narrow_dialog_returns_the_dialog_role() {
    // Arrange
    let dialog = dialog("about_dlg");

    // Act
    let least = least_width(dialog);

    // Assert
    assert_eq!(least, LeastWidth::Dialog);
}

#[test]
fn test_activation_given_no_close_returns_none() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(None, None, dialog);

    // Assert
    assert_eq!(found, None);
}

#[test]
fn test_activation_given_escape_returns_escape() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(Some(CloseReason::Escape), None, dialog);

    // Assert
    assert_eq!(found, Some(Activation::Escape));
}

#[test]
fn test_activation_given_a_close_action_returns_escape() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(Some(CloseReason::Requested), None, dialog);

    // Assert
    assert_eq!(found, Some(Activation::Escape));
}

#[test]
fn test_activation_given_an_unexpected_close_returns_escape() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(Some(CloseReason::Timeout), None, dialog);

    // Assert
    assert_eq!(found, Some(Activation::Escape));
}

#[test]
fn test_activation_given_enter_returns_the_default_button() {
    // Arrange
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(Some(CloseReason::Enter), None, dialog);

    // Assert
    assert_eq!(found, Some(Activation::Button(Button::Ok)));
}

#[test]
fn test_activation_given_enter_on_a_destructive_dialog_returns_cancel() {
    // Arrange: Enter never runs Delete Ride's destructive action.
    let dialog = dialog("delete_ride_dlg");

    // Act
    let found = activation(Some(CloseReason::Enter), None, dialog);

    // Assert
    assert_eq!(found, Some(Activation::Button(Button::Cancel)));
}

#[test]
fn test_activation_given_a_picked_index_returns_that_button() {
    // Arrange: Delete Ride lists Delete first and Cancel second.
    let dialog = dialog("delete_ride_dlg");

    // Act
    let found = activation(Some(CloseReason::Activated), Some(0), dialog);

    // Assert
    assert_eq!(found, Some(Activation::Button(Button::Delete)));
}

#[test]
fn test_activation_given_a_pick_without_a_close_returns_that_button() {
    // Arrange: a button that stays open still reports its pick.
    let dialog = dialog("ride_setup_dlg");

    // Act
    let found = activation(None, Some(1), dialog);

    // Assert
    assert_eq!(found, Some(Activation::Button(Button::Cancel)));
}

#[test]
fn test_activation_given_an_out_of_range_pick_returns_none() {
    // Arrange
    let dialog = dialog("about_dlg");

    // Act
    let found = activation(Some(CloseReason::Activated), Some(9), dialog);

    // Assert
    assert_eq!(found, None);
}

// ------------------------------------------------------------------ labels

#[test]
fn test_button_label_given_the_settings_ok_returns_ok() {
    // Arrange
    let state = AppState::blank();

    // Act
    let label = button_label(&state, "settings_dlg", Button::Ok);

    // Assert
    assert_eq!(label, "OK");
}

#[test]
fn test_button_label_given_the_ride_setup_ok_returns_save() {
    // Arrange
    let state = AppState::blank();

    // Act
    let label = button_label(&state, "ride_setup_dlg", Button::Ok);

    // Assert
    assert_eq!(label, "Save");
}

#[test]
fn test_button_label_given_the_add_rider_ok_returns_save() {
    // Arrange
    let state = AppState::blank();

    // Act
    let label = button_label(&state, "add_rider_dlg", Button::Ok);

    // Assert
    assert_eq!(label, "Save");
}

#[test]
fn test_least_width_given_the_settings_dialog_returns_the_wide_role() {
    // Arrange
    let dialog = dialog("settings_dlg");

    // Act
    let least = least_width(dialog);

    // Assert
    assert_eq!(least, LeastWidth::WideDialog);
}

// ------------------------------------------------------------------ controls

#[test]
fn test_control_sizing_given_a_form_control_returns_content() {
    // Arrange: craft's fill-width leaf measures against the form column, not the row cell, so a
    // form control takes its content width and stays inside its column.

    // Act
    let sizing = CONTROL_SIZING;

    // Assert
    assert_eq!(sizing, Sizing::Content);
}

/// Return a fixture showing one two-member radio group in a wide room.
fn radio_fixture() -> Fixture {
    let mut fixture = Fixture::new(|ui| {
        let _ = radio_group(
            ui,
            "Entry mode",
            0,
            &[
                (0, "solo_radio", "Solo"),
                (1, "mixed_radio", "Mixed (teams)"),
            ],
        );
    });
    fixture.room(Vec2::new(600.0, 300.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(2);
    fixture
}

#[test]
fn test_radio_group_places_its_members_in_one_row() {
    // Arrange
    let fixture = radio_fixture();

    // Act
    let radios = fixture.by_role(Role::RADIO_BUTTON);

    // Assert: both members share one vertical band.
    assert_eq!(radios.len(), 2);
    let solo = radios
        .first()
        .map(|target| target.rect)
        .expect("the Solo radio");
    let mixed = radios
        .get(1)
        .map(|target| target.rect)
        .expect("the Mixed radio");
    let shared = solo.min.y < mixed.max.y && mixed.min.y < solo.max.y;
    assert!(shared, "the two radios share one row: {solo:?} {mixed:?}");
}

#[test]
fn test_radio_group_puts_the_caption_above_the_members() {
    // Arrange
    let fixture = radio_fixture();

    // Act
    let caption = fixture
        .by_role(Role::LABEL)
        .into_iter()
        .find(|target| fixture.tree().name_of(target.node) == Some("Entry mode"))
        .expect("the group caption shows");
    let solo = fixture
        .by_role(Role::RADIO_BUTTON)
        .first()
        .map(|target| target.rect)
        .expect("the Solo radio");

    // Assert: the caption's band ends before the members' begins.
    assert!(
        caption.rect.max.y <= solo.min.y,
        "the caption stands above the radios: {:?} {solo:?}",
        caption.rect
    );
}

/// Return a fixture showing three captioned columns in one grid row.
fn triple_fixture() -> Fixture {
    let mut fixture = Fixture::new(|ui| {
        triple(ui, |first, second, third| {
            field_caption(first, "One");
            field_caption(second, "Two");
            field_caption(third, "Three");
        });
    });
    fixture.room(Vec2::new(900.0, 200.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(2);
    fixture
}

#[test]
fn test_triple_puts_its_three_fields_on_one_row() {
    // Arrange
    let fixture = triple_fixture();
    let rect = |label: &str| {
        fixture
            .by_role(Role::LABEL)
            .into_iter()
            .find(|target| fixture.tree().name_of(target.node) == Some(label))
            .map(|target| target.rect)
            .expect("the column caption shows")
    };

    // Act
    let one = rect("One");
    let two = rect("Two");
    let three = rect("Three");

    // Assert: one shared band, left to right.
    assert!(
        one.min.y < two.max.y && two.min.y < one.max.y,
        "{one:?} {two:?}"
    );
    assert!(
        two.min.y < three.max.y && three.min.y < two.max.y,
        "{two:?} {three:?}"
    );
    assert!(one.max.x <= two.min.x, "{one:?} {two:?}");
    assert!(two.max.x <= three.min.x, "{two:?} {three:?}");
}

/// The two density choices the select fixture offers.
const DENSITY_CHOICES: [(&str, &str); 2] = [("Compact", "compact"), ("Comfortable", "comfortable")];

/// Return a fixture showing one select after its caption in a wide room.
fn select_fixture() -> Fixture {
    let mut fixture = Fixture::new(|ui| {
        ui.horizontal(|ui| {
            field_caption(ui, "Density");
            let _ = select_value(
                ui,
                "density_choice",
                "Density",
                "comfortable",
                &DENSITY_CHOICES,
            );
        });
    });
    fixture.room(Vec2::new(600.0, 300.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(2);
    fixture
}

#[test]
fn test_decimal_value_given_a_width_stays_inside_its_cell() {
    // Arrange: a wide room, so a field that ignores its width would grow past the cell.
    let mut fixture = Fixture::new(|ui| {
        let mut value = 12.0;
        decimal_value(
            ui,
            "avg_speed",
            "Average speed",
            &mut value,
            220.0,
            0.0..=200.0,
        );
    });
    fixture.room(Vec2::new(600.0, 300.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());
    fixture.idle(2);

    // Act
    let field = fixture
        .by_role(Role::SPIN_BUTTON)
        .first()
        .map(|target| target.rect)
        .expect("the number field shows");

    // Assert
    assert!(
        field.width() <= 220.0,
        "the number field stays inside its cell: {field:?}"
    );
}

#[test]
fn test_select_value_stays_inside_its_row() {
    // Arrange
    let fixture = select_fixture();

    // Act
    let select = fixture
        .by_role(Role::COMBO_BOX)
        .first()
        .map(|target| target.rect)
        .expect("the select shows");

    // Assert: a content-sized select stops at its text and never overruns the row.
    assert!(
        select.width() <= 600.0,
        "the select stays inside its row: {select:?}"
    );
}

#[test]
fn test_message_spans_given_a_ride_name_make_it_strong() {
    // Arrange
    let message = "\"GORBA EPIC 2026\" is still running.";

    // Act
    let spans = message_spans(message, Some("GORBA EPIC 2026"));

    // Assert: only the ride name is the strong value.
    let strong: Vec<&str> = spans
        .iter()
        .filter(|span| span.strong)
        .map(|span| span.text.as_ref())
        .collect();
    assert_eq!(strong, ["GORBA EPIC 2026"]);
}

#[test]
fn test_message_spans_given_no_ride_name_returns_one_plain_span() {
    // Arrange
    let message = "Are you sure you want to quit?";

    // Act
    let spans = message_spans(message, None);

    // Assert
    assert_eq!(spans.len(), 1);
    assert!(!spans.first().is_some_and(|span| span.strong));
}

#[test]
fn test_message_body_given_a_ride_name_paints_it() {
    // Arrange
    let mut fixture = Fixture::new(|ui| {
        message_body(
            ui,
            Some("\"GORBA EPIC 2026\" is still running.".to_owned()),
            Some("GORBA EPIC 2026"),
        );
    });
    fixture.room(Vec2::new(480.0, 200.0));
    UiFonts::new(&I18nScripts).install(fixture.ctx());

    // Act
    fixture.idle(3);
    let shapes = fixture.frame_shapes();
    let galley = shapes
        .iter()
        .find_map(|shape| match shape {
            craft::egui::Shape::Text(text) => Some(&text.galley),
            _ => None,
        })
        .expect("the message paints");
    let mut faces = galley
        .job
        .sections
        .iter()
        .map(|section| &section.format.font_id);
    let body = faces.next();
    let strong = faces.any(|face| body.is_some_and(|body| face != body));

    // Assert: the ride name is painted in a face of its own, not the body's.
    assert!(
        galley.job.text.contains("GORBA EPIC 2026"),
        "{}",
        galley.job.text
    );
    assert!(strong, "the ride name reads as the emphasised value");
}

#[test]
fn test_message_spans_given_a_missing_ride_name_keeps_the_message_whole() {
    // Act
    let spans = message_spans("The ride is still open.", Some("GORBA EPIC 2026"));

    // Assert
    assert_eq!(spans.len(), 1);
    assert_eq!(
        spans.first().map(|span| span.text.as_ref()),
        Some("The ride is still open.")
    );
}

#[test]
fn test_message_spans_given_the_name_first_emits_no_empty_prefix() {
    // Act
    let spans = message_spans("GORBA EPIC 2026 is still running.", Some("GORBA EPIC 2026"));

    // Assert: the name's strong span, then the rest.
    assert_eq!(spans.len(), 2);
    assert!(spans.first().is_some_and(|span| span.strong));
}

#[test]
fn test_message_spans_given_the_name_last_emits_no_empty_suffix() {
    // Act
    let spans = message_spans("Your ride is GORBA EPIC 2026", Some("GORBA EPIC 2026"));

    // Assert
    assert_eq!(spans.len(), 2);
    assert_eq!(
        spans.first().map(|span| span.text.as_ref()),
        Some("Your ride is ")
    );
}
