// SPDX-License-Identifier: GPL-3.0-only

//! The correction forms and the Crossing Detail dialog.

use craft::egui::Ui;
use craft::ui::contract::Show as _;

use super::dialogs::{stack_date_field, stack_text_field, value};
use crate::ui::state::AppState;

/// Show the edit-crossing form (edit and add modes share it).
pub(crate) fn edit_crossing(ui: &mut Ui, state: &mut AppState) {
    stack_text_field(ui, state, "plate_input", "Plate");
    stack_text_field(ui, state, "time_picker", "Time (H:MM:SS)");
    stack_text_field(ui, state, "reason_input", "Reason");
}

/// Show the manual-deal form.
pub(crate) fn manual_deal(ui: &mut Ui, state: &mut AppState) {
    stack_text_field(ui, state, "plate_input", "Plate");
    stack_text_field(ui, state, "reason_input", "Reason");
}

/// Show the DNF form.
pub(crate) fn dnf(ui: &mut Ui, state: &mut AppState) {
    stack_text_field(ui, state, "plate_input", "Plate");
    ui.push_id("entry_lbl", |ui| {
        let _ =
            craft::ui::display::Text::new("The plate names the rider or entry that becomes DNF.")
                .show(ui);
    });
    stack_text_field(ui, state, "reason_input", "Reason");
}

/// Show the void-card form.
pub(crate) fn void_card(ui: &mut Ui, state: &mut AppState) {
    let card = state
        .crossing_detail
        .as_ref()
        .map_or_else(String::new, |detail| detail.model.card.clone());
    value(ui, "card_lbl", "Card", card);
    let entry = state
        .crossing_detail
        .as_ref()
        .map_or_else(String::new, |detail| detail.model.rider.clone());
    value(ui, "entry_lbl", "Entry", entry);
    stack_text_field(ui, state, "reason_input", "Reason");
}

/// Show the plate prompt.
pub(crate) fn crossing_number(ui: &mut Ui, state: &mut AppState) {
    stack_text_field(ui, state, "number_input", "Plate");
}

/// Show the set-start form.
pub(crate) fn set_start(ui: &mut Ui, state: &mut AppState) {
    stack_date_field(ui, state, "start_date_picker", "Start date");
    stack_text_field(ui, state, "start_time_picker", "Start time (H:MM:SS)");
}

/// Show the Crossing Detail's nine read-only values.
pub(crate) fn detail(ui: &mut Ui, state: &mut AppState) {
    let Some(detail) = state.crossing_detail.clone() else {
        return;
    };
    let cells = [
        ("crossing_rider_lbl", "Rider", &detail.model.rider),
        ("crossing_team_lbl", "Team", &detail.model.team),
        ("crossing_plate_lbl", "Plate", &detail.model.plate),
        ("crossing_lap_lbl", "Lap", &detail.model.lap),
        ("crossing_time_lbl", "Time", &detail.model.time),
        ("crossing_lap_time_lbl", "Lap time", &detail.model.lap_time),
        ("crossing_total_lbl", "Total", &detail.model.total),
        ("crossing_card_lbl", "Card", &detail.model.card),
        ("crossing_held_lbl", "Status", &detail.model.held),
    ];
    for chunk in cells.chunks(2) {
        ui.horizontal(|ui| {
            for (id, caption, text) in chunk {
                value(ui, id, caption, (*text).clone());
            }
        });
    }
}
