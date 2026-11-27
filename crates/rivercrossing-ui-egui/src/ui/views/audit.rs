// SPDX-License-Identifier: GPL-3.0-only

//! The audit viewer's search, action filter and rows.

use craft::egui::Ui;
use craft::ui::contract::{Bind, Show as _};
use craft::ui::text::TextField;

use super::dialogs::{
    ACTION, ENTRY, REASON, TIME, TableData, TableRow, columns, select_value, table,
};
use crate::ui::presenters::audit;
use crate::ui::state::AppState;

/// The audit rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 8;

/// Show the audit viewer.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.push_id("audit_search", |ui| {
            let _ =
                TextField::search("Search entry", Bind::bound(&mut state.audit.search)).show(ui);
        });
        let mut choices: Vec<(&'static str, &'static str)> =
            vec![(audit::ALL_ACTIONS, audit::ALL_ACTIONS)];
        choices.extend(audit::ACTION_CHOICES.iter().copied());
        let stored = state.audit.action.clone();
        if let Some(picked) = select_value(ui, "action_choice", "Action filter", &stored, &choices)
        {
            picked.clone_into(&mut state.audit.action);
        }
    });
    let data = TableData {
        columns: columns(&[
            (TIME, "When"),
            (ACTION, "Action"),
            (ENTRY, "Entry"),
            (REASON, "Reason"),
        ]),
        rows: state
            .filtered_audit()
            .iter()
            .map(|row| TableRow {
                label: format!("{} {}", row.at, row.entry),
                cells: vec![
                    (TIME, row.at.clone()),
                    (ACTION, row.action.clone()),
                    (ENTRY, row.entry.clone()),
                    (REASON, row.reason.clone()),
                ],
                ..TableRow::default()
            })
            .collect(),
    };
    let _ = table(ui, "audit_list", &data, VISIBLE_ROWS, "No audit rows", None);
}
