// SPDX-License-Identifier: GPL-3.0-only

//! The rider-issues report: the summary line, the issue list and the four one-click fixes.
//!
//! Each fix is gated by `presenters::rider_issues::actions`, so a row only offers the fixes its
//! kind and the roster's lock matrix allow. The card-sufficiency advisory (`card_check_lbl`) is
//! hidden while the ride's estimate is unavailable, exactly as the Python window hides it.

use craft::egui::Ui;
use craft::theme::Variant;
use craft::theme::variant::Tone;
use craft::ui::buttons::Button;
use craft::ui::contract::{Props as _, Show as _};
use craft::ui::display::Text;

use super::dialogs::{ISSUE, NAME, PLATE, TableData, TableRow, columns, notice, table};
use crate::ui::state::AppState;

/// The issue rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 6;

/// Show the rider-issues body.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let actions = state
        .rider_issues
        .selected
        .and_then(|index| state.rider_issues.rows.get(index))
        .and_then(|row| state.actions_for(row));
    let summary = format!("{} issues", state.rider_issues.rows.len());
    ui.push_id("issues_summary_lbl", |ui| {
        let _ = Text::new(summary).show(ui);
    });
    let rows = issue_rows(state);
    let selected = state.rider_issues.selected;
    if let Some(key) = table(
        ui,
        "issues_list",
        &rows,
        VISIBLE_ROWS,
        "No rider issues",
        selected,
    ) {
        state.rider_issues.selected = Some(key);
    }
    ui.horizontal(|ui| {
        ui.push_id("open_editor_btn", |ui| {
            let clicked = Button::new("Open Editor…")
                .variant(Variant::PRIMARY)
                .enabled(actions.is_some_and(|actions| actions.open_editor.is_some()))
                .show(ui)
                .clicked();
            if clicked {
                state.open_issue_editor();
            }
        });
        ui.push_id("convert_solo_btn", |ui| {
            let clicked = Button::new("Convert to Solo")
                .variant(Variant::SECONDARY)
                .enabled(actions.is_some_and(|actions| actions.convert_solo))
                .show(ui)
                .clicked();
            if clicked {
                let _ = state.convert_issue_to_solo();
            }
        });
        ui.push_id("assign_plate_btn", |ui| {
            let clicked = Button::new("Assign Plate")
                .variant(Variant::SECONDARY)
                .enabled(actions.is_some_and(|actions| actions.assign_plate))
                .show(ui)
                .clicked();
            if clicked {
                let _ = state.assign_issue_plate();
            }
        });
        ui.push_id("renumber_btn", |ui| {
            let clicked = Button::new("Renumber")
                .variant(Variant::SECONDARY)
                .enabled(actions.is_some_and(|actions| actions.renumber))
                .show(ui)
                .clicked();
            if clicked {
                let _ = state.renumber_issue();
            }
        });
    });
    if let Some(errors) = &state.rider_issues.errors {
        notice(ui, "issues_infobar", Tone::Bad, errors.clone());
    }
}

/// Return the issue rows as a table.
fn issue_rows(state: &AppState) -> TableData {
    let rows = state
        .rider_issues
        .rows
        .iter()
        .map(|row| TableRow {
            label: row.name.clone(),
            cells: vec![
                (PLATE, row.plate.clone()),
                (NAME, row.name.clone()),
                (ISSUE, row.detail.clone()),
            ],
        })
        .collect();
    TableData {
        columns: columns(&[(PLATE, "Plate"), (NAME, "Name"), (ISSUE, "Issue")]),
        rows,
    }
}
