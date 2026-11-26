// SPDX-License-Identifier: GPL-3.0-only

//! The rendered screens: the menu bar, the console, the ride-setup form and the dialog host.
//!
//! The views hold no decisions of their own: they read [`crate::ui::state::AppState`], show the
//! presenter results and forward each activation to the §15 action registry.

pub(crate) mod audit;
pub(crate) mod console;
pub(crate) mod corrections;
pub(crate) mod csv;
pub(crate) mod dialogs;
pub(crate) mod issues;
pub(crate) mod library;
pub(crate) mod menu;
pub(crate) mod riders;
pub(crate) mod selftest;
pub(crate) mod settings;
pub(crate) mod setup;
pub(crate) mod system;
pub(crate) mod teams;

use craft::egui::Ui;

/// Show one clickable list cell.
///
/// An empty cell shows an em dash, so the control always carries a non-empty accessible name.
pub(crate) fn cell(ui: &mut Ui, selected: bool, text: &str) -> bool {
    let shown = if text.trim().is_empty() { "—" } else { text };
    ui.selectable_label(selected, shown).clicked()
}

/// Show a plain text label.
///
/// The label is not selectable, so it declares no click action and needs no accessible name beyond
/// its own text.
pub(crate) fn text(ui: &mut Ui, text: &str) {
    ui.add(craft::egui::Label::new(text).selectable(false))
        .accessible_name(text);
}
