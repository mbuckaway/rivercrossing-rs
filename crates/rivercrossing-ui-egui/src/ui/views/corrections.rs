// SPDX-License-Identifier: GPL-3.0-only

//! The correction forms and the Crossing Detail dialog.

use craft::egui::Ui;
use craft::theme::Variant;
use craft::ui::buttons::Button;
use craft::ui::contract::{Bind, Props as _, Show as _, Sizing};
use craft::ui::text::TextField;

use super::dialogs::{field_caption, stack_date_field, stack_text_field_filled, value};
use crate::ui::state::AppState;

#[cfg(test)]
mod tests;

/// Show the edit-crossing form (edit and add modes share it).
///
/// Edit mode also carries the contract's `void_btn`, which opens the void-card flow for the
/// crossing the operator selected (`design/docs-md/control-mapping.md`, `edit_crossing_dlg`).
pub(crate) fn edit_crossing(ui: &mut Ui, state: &mut AppState) {
    stack_text_field_filled(ui, state, "plate_input", "Plate");
    stack_text_field_filled(ui, state, "time_picker", "Time (H:MM:SS)");
    stack_text_field_filled(ui, state, "reason_input", "Reason");
    void_action(ui, state);
}

/// Show the dialog's "Void card…" action when a selected crossing holds a credited card.
fn void_action(ui: &mut Ui, state: &mut AppState) {
    let can_void = !state.correction.add
        && state
            .crossing_detail
            .as_ref()
            .is_some_and(|detail| detail.card.is_some());
    if !can_void {
        return;
    }
    ui.push_id("void_btn", |ui| {
        let clicked = Button::new("Void card…")
            .variant(Variant::DANGER)
            .show(ui)
            .clicked();
        if clicked {
            state.open_void_card();
        }
    });
}

/// Show the manual-deal form.
pub(crate) fn manual_deal(ui: &mut Ui, state: &mut AppState) {
    stack_text_field_filled(ui, state, "plate_input", "Plate");
    stack_text_field_filled(ui, state, "reason_input", "Reason");
}

/// Show the DNF form: the plate or the entry the operator types names the target.
pub(crate) fn dnf(ui: &mut Ui, state: &mut AppState) {
    stack_text_field_filled(ui, state, "plate_input", "Plate");
    entry_field(ui, state);
    stack_text_field_filled(ui, state, "reason_input", "Reason");
}

/// Show the Entry field: a rider or team name, a plate, or a team key.
fn entry_field(ui: &mut Ui, state: &mut AppState) {
    ui.push_id("entry_lbl", |ui| {
        field_caption(ui, "Entry");
        let _ = TextField::new("Entry", Bind::bound(&mut state.correction_entry))
            .placeholder("Plate or entry")
            .sizing(Sizing::FillWidth)
            .show(ui);
    });
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
    stack_text_field_filled(ui, state, "reason_input", "Reason");
}

/// Show the plate prompt.
pub(crate) fn crossing_number(ui: &mut Ui, state: &mut AppState) {
    stack_text_field_filled(ui, state, "number_input", "Plate");
}

/// Show the set-start form.
pub(crate) fn set_start(ui: &mut Ui, state: &mut AppState) {
    stack_date_field(ui, state, "start_date_picker", "Start date");
    stack_text_field_filled(ui, state, "start_time_picker", "Start time (H:MM:SS)");
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
        // The design's `.grid2`: two equal columns, so every value starts on its column's edge
        // however wide its neighbour is.
        ui.columns(2, |columns| {
            for (column, cell) in columns.iter_mut().zip(chunk) {
                let (id, caption, text) = *cell;
                value(column, id, caption, text.clone());
            }
        });
    }
}
