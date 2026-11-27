// SPDX-License-Identifier: GPL-3.0-only

//! The About box and the Keyboard Shortcuts list, generated from the action registry.

use craft::egui::Ui;
use craft::ui::buttons::Link;
use craft::ui::contract::Show as _;
use craft::ui::display::Text;

use super::dialogs::{NAME, SHORTCUT, TableData, TableRow, columns, field_caption, table, value};
use crate::ui::action::spec::ACTIONS;

#[cfg(test)]
mod tests;

/// The product version the About box names.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The product description line.
const DESCRIPTION: &str = "Timing & poker-hand scoring for poker-run rides.";

/// The author line.
const AUTHOR: &str = "Storytold";

/// The copyright line.
const COPYRIGHT: &str = "Copyright GORBA";

/// The licence line.
const LICENSE: &str = "GPL-3.0-only";

/// The organizer site the About box links to.
const GORBA_URL: &str = "https://gorba.example";

/// The shortcut rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 20;

/// Show the About box.
pub(crate) fn about(ui: &mut Ui) {
    // The design's `.stat` rows (`design/mockups/index.html`, `#s-about`): a caption over its value.
    value(ui, "version_lbl", "Version", VERSION);
    let _ = Text::new(DESCRIPTION).show(ui);
    value(ui, "author_lbl", "Author", AUTHOR);
    value(ui, "copyright_lbl", "Copyright", COPYRIGHT);
    value(ui, "license_lbl", "License", LICENSE);
    url_row(ui);
}

/// Show the organizer link as a stat row.
fn url_row(ui: &mut Ui) {
    ui.push_id("gorba_link", |ui| {
        ui.vertical(|ui| {
            field_caption(ui, "URL");
            let _ = Link::new(GORBA_URL).url(GORBA_URL).show(ui);
        });
    });
}

/// Show the shortcut table, generated from the registry.
pub(crate) fn shortcuts(ui: &mut Ui) {
    let data = shortcut_table();
    let _ = table(
        ui,
        "shortcuts_list",
        &data,
        VISIBLE_ROWS,
        "No shortcuts",
        None,
    );
}

/// Return the two-column shortcut table: one row per action the registry documents a shortcut for.
fn shortcut_table() -> TableData {
    TableData {
        columns: columns(&[(NAME, "Command"), (SHORTCUT, "Shortcut")]),
        rows: ACTIONS
            .iter()
            .filter_map(|row| {
                let shortcut = row.shortcut?;
                Some(TableRow {
                    label: row.label.to_owned(),
                    // The label carries the command name into its column; a NAME cell would repeat
                    // it.
                    cells: vec![(SHORTCUT, shortcut.text())],
                    ..TableRow::default()
                })
            })
            .collect(),
    }
}
