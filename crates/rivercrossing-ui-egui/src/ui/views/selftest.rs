// SPDX-License-Identifier: GPL-3.0-only

//! The evaluator self-test's read-only canvas output.

use craft::egui::{self, Ui};

use super::text;
use crate::ui::state::AppState;

/// Show the self-test lines.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    ui.push_id("selftest_output", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("selftest_scroll")
            .max_height(320.0)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    for line in &state.selftest_lines {
                        text(ui, line);
                    }
                });
            });
    });
}
