// SPDX-License-Identifier: GPL-3.0-only

//! The rendered screens: the menu bar, the console, the ride-setup form and the dialog host.
//!
//! The views hold no decisions of their own: they read [`crate::ui::state::AppState`], show the
//! presenter results and forward each activation to the §15 action registry.

pub(crate) mod console;
pub(crate) mod dialogs;
pub(crate) mod menu;
pub(crate) mod setup;

use craft::egui::Ui;

/// Show a plain text label.
///
/// The label is not selectable, so it declares no click action and needs no accessible name beyond
/// its own text.
pub(crate) fn text(ui: &mut Ui, text: &str) {
    ui.add(craft::egui::Label::new(text).selectable(false))
        .accessible_name(text);
}
