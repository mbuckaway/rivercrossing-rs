// SPDX-License-Identifier: GPL-3.0-only

//! The `ride_setup_dlg` form: the ride's fields, the entry and plate-model radios, the shoe
//! options and the card cap, collected into [`RideConfigParams`] on OK.
//!
//! Every field writes back into the open dialog's frozen field map, so the automation channel's
//! `ui.set` and the form's own widgets address the same value. The infobar shows the presenter's
//! refusal messages and the dialog stays open for the correction.

use craft::egui::Ui;
use craft::theme::variant::Tone;
use craft::ui::contract::ShowContainer as _;
use craft::ui::sections::Section;

use super::dialogs::{
    notice, radio_group, stack_date_field, stack_decimal_field, stack_select, stack_text_field,
    stack_whole_field,
};
use crate::ui::state::AppState;

/// The `cap_choice` items: no cap, then one per cap in 5..=20.
const CAP_CHOICES: [(&str, &str); 17] = [
    ("Disabled", "Disabled"),
    ("5", "5"),
    ("6", "6"),
    ("7", "7"),
    ("8", "8"),
    ("9", "9"),
    ("10", "10"),
    ("11", "11"),
    ("12", "12"),
    ("13", "13"),
    ("14", "14"),
    ("15", "15"),
    ("16", "16"),
    ("17", "17"),
    ("18", "18"),
    ("19", "19"),
    ("20", "20"),
];

/// The default lap length in kilometres, matching the presenter's New Ride defaults.
const DEFAULT_LAP_KM: f64 = 8.0;

/// The default shoe size, matching the presenter's New Ride defaults.
const DEFAULT_DECKS: f64 = 8.0;

/// The default joker count, matching the presenter's New Ride defaults.
const DEFAULT_JOKERS: f64 = 1.0;

/// Show the ride-setup form.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    infobar(ui, state);
    let _ = Section::fixed("Ride").show(ui, |ui, _| ride_group(ui, state));
    let _ = Section::fixed("Format").show(ui, |ui, _| format_group(ui, state));
    let _ = Section::fixed("Shoe").show(ui, |ui, _| shoe_group(ui, state));
}

/// Show the event identity fields, two to a row as the design lays them out.
fn ride_group(ui: &mut Ui, state: &mut AppState) {
    pair(ui, |left, right| {
        stack_text_field(left, state, "name_input", "Ride name");
        stack_date_field(right, state, "date_picker", "Date");
    });
    pair(ui, |left, right| {
        stack_text_field(left, state, "start_time_picker", "Start time");
        stack_text_field(right, state, "venue_input", "Venue");
    });
    pair(ui, |left, right| {
        stack_text_field(left, state, "organizer_input", "Organizer");
        stack_text_field(right, state, "scorer_input", "Scorer");
    });
    pair(ui, |left, right| {
        stack_decimal_field(
            left,
            state,
            "lap_km_spin",
            "Lap length (km)",
            DEFAULT_LAP_KM,
            0.1..=100.0,
        );
        stack_text_field(right, state, "duration_input", "Duration (H:MM)");
    });
    pair(ui, |left, right| {
        stack_text_field(left, state, "min_lap_input", "Min lap (M:SS)");
        stack_select(right, state, "cap_choice", "Hand cap", &CAP_CHOICES);
    });
}

/// Show two fields side by side: the design's two-column form (`design/mockups/index.html`,
/// `.grid2`).
fn pair(ui: &mut Ui, content: impl FnOnce(&mut Ui, &mut Ui)) {
    ui.columns(2, |columns| {
        let (left, right) = columns.split_at_mut(1);
        if let (Some(left), Some(right)) = (left.first_mut(), right.first_mut()) {
            content(left, right);
        }
    });
}

/// Show the entry and plate-model choices, two to a row.
fn format_group(ui: &mut Ui, state: &mut AppState) {
    pair(ui, |left, right| {
        radio_pair(
            left,
            state,
            "Entry mode",
            ("solo_radio", "Solo"),
            ("mixed_radio", "Mixed (teams)"),
        );
        stack_whole_field(
            right,
            state,
            "team_size_spin",
            "Largest team",
            4.0,
            2.0..=9.0,
        );
    });
    radio_pair(
        ui,
        state,
        "Plate model",
        ("pooled_radio", "Rider plates (pooled)"),
        ("relay_radio", "Team relay"),
    );
    radio_pair(
        ui,
        state,
        "Short laps",
        ("hold_short_radio", "Hold short-lap cards for review"),
        ("always_deal_radio", "Always deal cards"),
    );
}

/// Show the shoe's options, two to a row where the design pairs them.
fn shoe_group(ui: &mut Ui, state: &mut AppState) {
    pair(ui, |left, right| {
        stack_whole_field(
            left,
            state,
            "decks_spin",
            "Decks",
            DEFAULT_DECKS,
            1.0..=20.0,
        );
        stack_whole_field(
            right,
            state,
            "jokers_spin",
            "Jokers",
            DEFAULT_JOKERS,
            0.0..=9.0,
        );
    });
    radio_pair(
        ui,
        state,
        "Jokers counted",
        ("jokers_per_deck_radio", "Jokers per deck"),
        ("jokers_total_radio", "Jokers total"),
    );
    stack_select(ui, state, "cap_choice", "Card cap", &CAP_CHOICES);
}

/// Show the dialog's refusal messages.
fn infobar(ui: &mut Ui, state: &AppState) {
    let Some(errors) = &state.setup_errors else {
        return;
    };
    notice(ui, "setup_infobar", Tone::Bad, errors.join("; "));
}

/// Show a pair of radios over two frozen controls; picking one clears the other.
fn radio_pair(
    ui: &mut Ui,
    state: &mut AppState,
    group: &'static str,
    first: (&'static str, &'static str),
    second: (&'static str, &'static str),
) {
    let picked = u8::from(state.dialogs.field(second.0) == Some("true"));
    let chosen = radio_group(
        ui,
        group,
        picked,
        &[(0, first.0, first.1), (1, second.0, second.1)],
    );
    if chosen == picked {
        return;
    }
    let _ = state.dialogs.set_field(first.0, bool_word(chosen == 0));
    let _ = state.dialogs.set_field(second.0, bool_word(chosen == 1));
}

/// Return the stored word of a flag.
fn bool_word(on: bool) -> &'static str {
    if on { "true" } else { "false" }
}
