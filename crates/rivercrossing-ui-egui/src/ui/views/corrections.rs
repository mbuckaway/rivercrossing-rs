// SPDX-License-Identifier: GPL-3.0-only

//! The correction forms and the Crossing Detail dialog.

use craft::egui::{self, Ui};

use super::dialogs::stack_text_field;
use super::text;
use crate::ui::state::AppState;

/// Show the edit-crossing form (edit and add modes share it).
pub(crate) fn edit_crossing(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    stack_text_field(ui, state, "plate_input", "Plate", focused);
    stack_text_field(ui, state, "time_picker", "Time (H:MM:SS)", focused);
    stack_text_field(ui, state, "reason_input", "Reason", focused);
}

/// Show the manual-deal form.
pub(crate) fn manual_deal(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    stack_text_field(ui, state, "plate_input", "Plate", focused);
    stack_text_field(ui, state, "reason_input", "Reason", focused);
}

/// Show the DNF form.
pub(crate) fn dnf(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    stack_text_field(ui, state, "plate_input", "Plate", focused);
    ui.push_id("entry_lbl", |ui| {
        text(ui, "The plate names the rider or entry that becomes DNF.");
    });
    stack_text_field(ui, state, "reason_input", "Reason", focused);
}

/// Show the void-card form.
pub(crate) fn void_card(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    let card = state
        .crossing_detail
        .as_ref()
        .map_or_else(String::new, |detail| detail.model.card.clone());
    ui.push_id("card_lbl", |ui| text(ui, &card));
    let entry = state
        .crossing_detail
        .as_ref()
        .map_or_else(String::new, |detail| detail.model.rider.clone());
    ui.push_id("entry_lbl", |ui| text(ui, &entry));
    stack_text_field(ui, state, "reason_input", "Reason", focused);
}

/// Show the plate prompt.
pub(crate) fn crossing_number(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    stack_text_field(ui, state, "number_input", "Plate", focused);
}

/// Show the set-start form.
pub(crate) fn set_start(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    stack_text_field(ui, state, "start_date_picker", "Date (YYYY-MM-DD)", focused);
    stack_text_field(
        ui,
        state,
        "start_time_picker",
        "Start time (H:MM:SS)",
        focused,
    );
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
    egui::Grid::new("crossing_detail_grid")
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            for chunk in cells.chunks(2) {
                for (id, caption, value) in chunk {
                    ui.push_id(*id, |ui| {
                        ui.vertical(|ui| {
                            text(ui, caption);
                            text(ui, value);
                        });
                    });
                }
                if chunk.len() == 1 {
                    text(ui, "");
                    text(ui, "");
                }
                ui.end_row();
            }
        });
}
