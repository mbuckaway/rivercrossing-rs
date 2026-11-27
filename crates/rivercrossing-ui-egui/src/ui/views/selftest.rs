// SPDX-License-Identifier: GPL-3.0-only

//! The evaluator self-test's read-only mono output.

use craft::egui::Ui;
use craft::ui::contract::Show as _;
use craft::ui::display::Text;
use craft::ui::style::TypeRole;

use crate::ui::state::AppState;

/// Show the self-test lines.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    ui.push_id("selftest_output", |ui| {
        for line in &state.selftest_lines {
            let _ = Text::new(line.clone()).role(TypeRole::Mono).show(ui);
        }
    });
}
