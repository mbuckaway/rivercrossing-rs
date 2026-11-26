// SPDX-License-Identifier: GPL-3.0-only

//! The rider-issues report: the summary line, the issue list and the three one-click fixes.
//!
//! Each fix is gated by `presenters::rider_issues::actions`, so a row only offers the fixes its
//! kind and the roster's lock matrix allow. The card-sufficiency advisory (`card_check_lbl`) is
//! hidden while the ride's estimate is unavailable, exactly as the Python window hides it.

use craft::egui::{self, Ui};

use super::text;
use crate::ui::state::AppState;

/// Show the rider-issues body.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let actions = state
        .rider_issues
        .selected
        .and_then(|index| state.rider_issues.rows.get(index))
        .and_then(|row| state.actions_for(row));
    let summary = format!("{} issues", state.rider_issues.rows.len());
    ui.push_id("issues_summary_lbl", |ui| text(ui, &summary));
    let mut clicked = None;
    ui.push_id("issues_list", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("issues_list_scroll")
            .max_height(220.0)
            .show(ui, |ui| {
                egui::Grid::new("issues_grid")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        for heading in ["Plate", "Name", "Issue"] {
                            text(ui, heading);
                        }
                        ui.end_row();
                        for (index, row) in state.rider_issues.rows.iter().enumerate() {
                            let selected_row = state.rider_issues.selected == Some(index);
                            for cell in [&row.plate, &row.name, &row.detail] {
                                if super::cell(ui, selected_row, cell) {
                                    clicked = Some(index);
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
    });
    if let Some(index) = clicked {
        state.rider_issues.selected = Some(index);
    }
    ui.horizontal(|ui| {
        let open = actions.is_some_and(|actions| actions.open_editor.is_some());
        if ui
            .push_id("open_editor_btn", |ui| {
                ui.add_enabled(open, egui::Button::new("Open Editor…"))
            })
            .inner
            .clicked()
        {
            state.open_issue_editor();
        }
        if ui
            .push_id("convert_solo_btn", |ui| {
                ui.add_enabled(
                    actions.is_some_and(|actions| actions.convert_solo),
                    egui::Button::new("Convert to Solo"),
                )
            })
            .inner
            .clicked()
        {
            let _ = state.convert_issue_to_solo();
        }
        if ui
            .push_id("assign_plate_btn", |ui| {
                ui.add_enabled(
                    actions.is_some_and(|actions| actions.assign_plate),
                    egui::Button::new("Assign Plate"),
                )
            })
            .inner
            .clicked()
        {
            let _ = state.assign_issue_plate();
        }
        if ui
            .push_id("renumber_btn", |ui| {
                ui.add_enabled(
                    actions.is_some_and(|actions| actions.renumber),
                    egui::Button::new("Renumber"),
                )
            })
            .inner
            .clicked()
        {
            let _ = state.renumber_issue();
        }
    });
    if let Some(errors) = &state.rider_issues.errors {
        ui.push_id("issues_infobar", |ui| text(ui, errors));
    }
}
