// SPDX-License-Identifier: GPL-3.0-only

//! The `ride_setup_dlg` form: the ride's fields, the entry and plate-model radios, the shoe
//! options and the card cap, collected into [`RideConfigParams`] on OK.
//!
//! Every field writes back into the open dialog's frozen field map, so the automation channel's
//! `ui.set` and the form's own widgets address the same value. The infobar shows the presenter's
//! refusal messages and the dialog stays open for the correction.

use craft::egui::{self, DragValue, TextEdit, Ui};

use super::text;
use crate::ui::state::AppState;

/// The stored `cap_choice` item that means no cap.
const CAP_DISABLED: &str = "Disabled";

/// The lowest card cap the choice offers.
const CAP_MIN: u32 = 5;

/// The highest card cap the choice offers.
const CAP_MAX: u32 = 20;

/// The default lap length in kilometres, matching the presenter's New Ride defaults.
const DEFAULT_LAP_KM: f64 = 8.0;

/// Show the ride-setup form.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    ui.heading("Ride").accessible_name("Ride");
    text_field(ui, state, "name_input", focused, "Ride name");
    text_field(ui, state, "date_picker", focused, "Date (YYYY-MM-DD)");
    text_field(
        ui,
        state,
        "start_time_picker",
        focused,
        "Start time (HH:MM)",
    );
    text_field(ui, state, "venue_input", focused, "Venue");
    text_field(ui, state, "organizer_input", focused, "Organizer");
    text_field(ui, state, "scorer_input", focused, "Scorer");
    spin_decimal(
        ui,
        state,
        "lap_km_spin",
        focused,
        "Lap length (km)",
        DEFAULT_LAP_KM,
        0.1..=100.0,
    );
    text_field(ui, state, "duration_input", focused, "Duration (H:MM)");
    text_field(ui, state, "min_lap_input", focused, "Min lap (M:SS)");
    ui.heading("Cards").accessible_name("Cards");
    radio_pair(
        ui,
        state,
        ("hold_short_radio", "Hold short-lap cards for review"),
        ("always_deal_radio", "Always deal cards"),
    );
    ui.heading("Entries").accessible_name("Entries");
    radio_pair(
        ui,
        state,
        ("solo_radio", "Solo"),
        ("mixed_radio", "Mixed (teams)"),
    );
    spin_whole(
        ui,
        state,
        "team_size_spin",
        focused,
        "Largest team",
        4,
        2..=9,
    );
    radio_pair(
        ui,
        state,
        ("pooled_radio", "Rider plates (pooled)"),
        ("relay_radio", "Team relay"),
    );
    ui.heading("Shoe").accessible_name("Shoe");
    spin_whole(ui, state, "decks_spin", focused, "Decks", 8, 1..=20);
    spin_whole(ui, state, "jokers_spin", focused, "Jokers", 1, 0..=9);
    radio_pair(
        ui,
        state,
        ("jokers_per_deck_radio", "Jokers per deck"),
        ("jokers_total_radio", "Jokers total"),
    );
    cap_choice(ui, state, focused);
    infobar(ui, state);
}

/// Show the dialog's refusal messages.
fn infobar(ui: &mut Ui, state: &AppState) {
    let Some(errors) = &state.setup_errors else {
        return;
    };
    ui.push_id("setup_infobar", |ui| {
        text(ui, &errors.join("; "));
    });
}

/// Show one text field and write its edit back into the dialog's field map.
fn text_field(ui: &mut Ui, state: &mut AppState, id: &str, focused: Option<&str>, label: &str) {
    let mut text = state.dialogs.field(id).unwrap_or_default().to_owned();
    let response = ui
        .push_id(id, |ui| {
            ui.add(
                TextEdit::singleline(&mut text)
                    .hint_text(label)
                    .desired_width(260.0),
            )
            .accessible_name(label)
        })
        .inner;
    if focused == Some(id) {
        super::dialogs::focus_ring(ui, &response);
    }
    if response.changed() {
        let _ = state.dialogs.set_field(id, text);
    }
}

/// Show a pair of radios; picking one clears the other.
fn radio_pair(ui: &mut Ui, state: &mut AppState, first: (&str, &str), second: (&str, &str)) {
    ui.horizontal(|ui| {
        for (id, label, other) in [(first.0, first.1, second.0), (second.0, second.1, first.0)] {
            let selected = state.dialogs.field(id) == Some("true");
            let clicked = ui
                .push_id(id, |ui| ui.radio(selected, label).clicked())
                .inner;
            if clicked {
                let _ = state.dialogs.set_field(id, "true");
                let _ = state.dialogs.set_field(other, "false");
            }
        }
    });
}

/// Show one whole-number spinner and write its edit back into the dialog's field map.
fn spin_whole(
    ui: &mut Ui,
    state: &mut AppState,
    id: &str,
    focused: Option<&str>,
    label: &str,
    default: i32,
    range: std::ops::RangeInclusive<i32>,
) {
    let mut value = state
        .dialogs
        .field(id)
        .and_then(|text| text.parse().ok())
        .unwrap_or(default);
    let response = ui
        .push_id(id, |ui| {
            ui.add(
                DragValue::new(&mut value)
                    .range(range)
                    .prefix(format!("{label}: ")),
            )
            .accessible_name(label)
        })
        .inner;
    if focused == Some(id) {
        super::dialogs::focus_ring(ui, &response);
    }
    if response.changed() {
        let _ = state.dialogs.set_field(id, value.to_string());
    }
}

/// Show one decimal spinner and write its edit back into the dialog's field map.
fn spin_decimal(
    ui: &mut Ui,
    state: &mut AppState,
    id: &str,
    focused: Option<&str>,
    label: &str,
    default: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    let mut value = state
        .dialogs
        .field(id)
        .and_then(|text| text.parse().ok())
        .unwrap_or(default);
    let response = ui
        .push_id(id, |ui| {
            ui.add(
                DragValue::new(&mut value)
                    .range(range)
                    .speed(0.1)
                    .prefix(format!("{label}: ")),
            )
            .accessible_name(label)
        })
        .inner;
    if focused == Some(id) {
        super::dialogs::focus_ring(ui, &response);
    }
    if response.changed() {
        let _ = state.dialogs.set_field(id, value.to_string());
    }
}

/// Show the card-cap choice: `Disabled`, then one item per cap in 5..=20.
fn cap_choice(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    let current = state
        .dialogs
        .field("cap_choice")
        .unwrap_or(CAP_DISABLED)
        .to_owned();
    let mut selected = current.clone();
    ui.horizontal(|ui| {
        super::text(ui, "Card cap");
        let response = ui
            .push_id("cap_choice", |ui| {
                egui::ComboBox::from_id_salt("cap_choice")
                    .selected_text(&selected)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected, CAP_DISABLED.to_owned(), CAP_DISABLED);
                        for cap in CAP_MIN..=CAP_MAX {
                            ui.selectable_value(&mut selected, cap.to_string(), cap.to_string());
                        }
                    })
                    .response
            })
            .inner;
        let response = response.accessible_name("Card cap");
        if focused == Some("cap_choice") {
            super::dialogs::focus_ring(ui, &response);
        }
    });
    if selected != current {
        let _ = state.dialogs.set_field("cap_choice", selected);
    }
}
