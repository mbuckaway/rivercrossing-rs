// SPDX-License-Identifier: GPL-3.0-only

//! The `WordPress` publish form and its progress window.

use craft::egui::Ui;
use craft::theme::variant::Tone;
use craft::ui::contract::{Props as _, Show as _, ShowContainer as _};
use craft::ui::display::{Progress, Text};
use craft::ui::sections::Section;

use super::dialogs::{field_caption, notice, select_value, stack_secret_field, stack_text_field};
use crate::ui::presenters::publish_wordpress::{
    PUBLISH_KIND_CHOICES, STATUS_CHOICES, resolved_kind, status_value,
};
use crate::ui::state::AppState;

/// Show the publish form.
pub(crate) fn form_body(ui: &mut Ui, state: &mut AppState) {
    kind(ui, state);
    let _ = Section::fixed("Site").show(ui, |ui, _| {
        stack_text_field(ui, state, "wp_url_input", "Site URL");
        stack_text_field(ui, state, "wp_username_input", "Username");
        stack_secret_field(ui, state, "wp_password_input", "Application password");
    });
    let _ = Section::fixed("Page").show(ui, |ui, _| {
        stack_text_field(ui, state, "wp_title_input", "Page title");
        stack_text_field(ui, state, "wp_slug_input", "Slug");
        stack_text_field(ui, state, "wp_parent_input", "Parent page id");
        status(ui, state);
    });
    if let Some(errors) = state.publish_errors.clone() {
        notice(ui, "publish_infobar", Tone::Bad, errors.join("; "));
    }
}

/// Show the publish-kind choice; a new kind re-fills the page title and slug.
fn kind(ui: &mut Ui, state: &mut AppState) {
    let stored = state
        .dialogs
        .field("wp_kind_choice")
        .unwrap_or_default()
        .to_owned();
    let selected = resolved_kind(&stored);
    field_caption(ui, "What to publish");
    if let Some(picked) = select_value(
        ui,
        "wp_kind_choice",
        "What to publish",
        selected,
        &PUBLISH_KIND_CHOICES,
    ) {
        state.set_publish_kind(picked);
    }
}

/// Show the page-status choice.
fn status(ui: &mut Ui, state: &mut AppState) {
    let stored = state
        .dialogs
        .field("wp_status_choice")
        .unwrap_or_default()
        .to_owned();
    let selected = status_value(&stored);
    field_caption(ui, "Status");
    if let Some(picked) = select_value(ui, "wp_status_choice", "Status", selected, &STATUS_CHOICES)
    {
        let _ = state.dialogs.set_field("wp_status_choice", picked);
    }
}

/// Show the publish progress window: an indeterminate bar, since the worker reports no fraction.
pub(crate) fn running_body(ui: &mut Ui, state: &AppState) {
    ui.push_id("publish_status_lbl", |ui| {
        let _ = Text::new(state.publish_status.clone()).show(ui);
    });
    ui.push_id("progress_gauge", |ui| {
        let _ = Progress::indeterminate()
            .label("Publishing progress")
            .show(ui);
    });
}
