// SPDX-License-Identifier: GPL-3.0-only

//! The `WordPress` publish form and its progress window.

use craft::egui::{self, ComboBox, Ui};

use super::dialogs::stack_text_field;
use super::text;
use crate::ui::presenters::publish_wordpress::{
    PUBLISH_KIND_CHOICES, STATUS_CHOICES, resolved_kind, status_value,
};
use crate::ui::state::AppState;

/// Show the publish form.
pub(crate) fn form_body(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    choice(ui, state, "wp_kind_choice", "Kind", &PUBLISH_KIND_CHOICES);
    for (field, label) in [
        ("wp_url_input", "Site URL"),
        ("wp_username_input", "Username"),
        ("wp_password_input", "Application password"),
        ("wp_title_input", "Page title"),
        ("wp_slug_input", "Slug"),
        ("wp_parent_input", "Parent page id"),
    ] {
        stack_text_field(ui, state, field, label, focused);
    }
    choice(ui, state, "wp_status_choice", "Status", &STATUS_CHOICES);
    if let Some(errors) = state.publish_errors.clone() {
        ui.push_id("publish_infobar", |ui| text(ui, &errors.join("; ")));
    }
}

/// Show a label and a choice bound to one dialog field.
fn choice(
    ui: &mut Ui,
    state: &mut AppState,
    id: &str,
    label: &str,
    choices: &[(&'static str, &'static str)],
) {
    let stored = state.dialogs.field(id).unwrap_or_default().to_owned();
    let selected = if id == "wp_kind_choice" {
        resolved_kind(&stored)
    } else {
        status_value(&stored)
    };
    let shown = choices
        .iter()
        .find(|(_label, value)| *value == selected)
        .map_or_else(
            || choices.first().map_or("", |(label, _value)| *label),
            |(label, _value)| *label,
        );
    let mut picked = stored.clone();
    ui.push_id(id, |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(label).selectable(false))
                .accessible_name(label);
            ComboBox::from_id_salt(format!("{id}_combo"))
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (choice_label, value) in choices {
                        ui.selectable_value(&mut picked, (*value).to_owned(), *choice_label);
                    }
                })
                .response
                .accessible_name(label);
        });
    });
    if picked != stored {
        if id == "wp_kind_choice" {
            state.set_publish_kind(&picked);
        } else {
            let _ = state.dialogs.set_field(id, picked);
        }
    }
}

/// Show the publish progress window.
pub(crate) fn running_body(ui: &mut Ui, state: &AppState) {
    ui.push_id("publish_status_lbl", |ui| text(ui, &state.publish_status));
    ui.push_id("progress_gauge", |ui| {
        ui.add(egui::ProgressBar::new(0.5).animate(true))
            .accessible_name("Publishing progress");
    });
}
