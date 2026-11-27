// SPDX-License-Identifier: GPL-3.0-only

//! The About box and the Keyboard Shortcuts list, generated from the action registry.

use craft::egui::Ui;
use craft::ui::buttons::Link;
use craft::ui::contract::Show as _;
use craft::ui::display::Text;

use super::dialogs::{NAME, SHORTCUT, TableData, TableRow, columns, table};
use crate::ui::action::spec::{ACTIONS, Shortcut};

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
    line(ui, "version_lbl", format!("RiverCrossing {VERSION}"));
    let _ = Text::new(DESCRIPTION).show(ui);
    line(ui, "author_lbl", AUTHOR.to_owned());
    line(ui, "copyright_lbl", COPYRIGHT.to_owned());
    line(ui, "license_lbl", LICENSE.to_owned());
    ui.push_id("gorba_link", |ui| {
        let _ = Link::new(GORBA_URL).url(GORBA_URL).show(ui);
    });
}

/// Show one About line under its frozen id.
fn line(ui: &mut Ui, id: &'static str, text: String) {
    ui.push_id(id, |ui| {
        let _ = Text::new(text).show(ui);
    });
}

/// Show the shortcut table, generated from the registry.
pub(crate) fn shortcuts(ui: &mut Ui) {
    let data = TableData {
        columns: columns(&[(NAME, "Command"), (SHORTCUT, "Shortcut")]),
        rows: ACTIONS
            .iter()
            .map(|row| TableRow {
                label: row.label.to_owned(),
                // The label carries the command name into its column; a NAME cell would repeat it.
                cells: vec![(
                    SHORTCUT,
                    row.shortcut.map_or_else(String::new, Shortcut::text),
                )],
            })
            .collect(),
    };
    let _ = table(
        ui,
        "shortcuts_list",
        &data,
        VISIBLE_ROWS,
        "No shortcuts",
        None,
    );
}
