// SPDX-License-Identifier: GPL-3.0-only

//! The dialog host: the top [`DialogStack`] dialog shown over an `egui::Modal`.
//!
//! The title, the fields and the buttons in focus order come from the frozen
//! [`crate::ui::dialogs::spec`] contract. Esc routes through the stack's cancel, Enter through its
//! default, and a clicked button reports its own id through [`DialogStack::activate`]. The ride
//! setup's OK validates its form and hands the config to the app; every other dialog only reports
//! its event until its own screen lands in a later release.

use craft::egui::{self, Context, Key, Modifiers, Response, Stroke, Ui};

use super::text;
use super::{
    audit, corrections, csv, issues, library, publish, results, riders, selftest, settings, setup,
    simulation, system, teams,
};
use crate::ui::dialogs::spec::{self, Button, DialogSpec};
use crate::ui::dialogs::stack::{self, Focus};
use crate::ui::ids;
use crate::ui::state::{AppState, NativeTask};

/// What closed the dialog this pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Activation {
    /// A clicked button.
    Button(Button),
    /// Enter, the default button.
    Enter,
    /// Esc, the cancel button.
    Escape,
}

/// Show the top dialog, when one is open.
pub(crate) fn show(ctx: &Context, state: &mut AppState) {
    let Some(top) = state.dialogs.top().cloned() else {
        return;
    };
    let Some(spec) = spec::spec(top.id) else {
        return;
    };
    let focused = match &top.focus {
        Focus::Field(field) => Some(*field),
        Focus::Button(_) => None,
    };
    let mut activation: Option<Activation> = None;
    let ctx = ctx.clone();
    let modal = egui::Modal::new(egui::Id::unique(top.id)).show(&ctx, |ui| {
        ui.set_min_width(480.0);
        let title = dialog_title(state, spec);
        ui.heading(title).accessible_name(title);
        match top.id {
            "ride_setup_dlg" => setup::body(ui, state, focused),
            "rider_editor_dlg" => riders::editor_body(ui, state),
            "add_rider_dlg" => riders::form_body(ui, state, focused),
            "team_editor_dlg" => teams::editor_body(ui, state),
            "add_team_dlg" => teams::form_body(ui, state, focused),
            "csv_preview_dlg" => csv::body(ui, state),
            "rider_issues_dlg" => issues::body(ui, state),
            "ride_library_dlg" => library::body(ui, state),
            "delete_ride_dlg" => library::delete_body(ui, state, focused),
            "audit_dlg" => audit::body(ui, state),
            "settings_dlg" => settings::body(ui, state),
            "selftest_dlg" => selftest::body(ui, state),
            "about_dlg" => system::about(ui, state),
            "shortcuts_dlg" => system::shortcuts(ui, state),
            "edit_crossing_dlg" => corrections::edit_crossing(ui, state, focused),
            "manual_deal_dlg" => corrections::manual_deal(ui, state, focused),
            "dnf_confirm_dlg" => corrections::dnf(ui, state, focused),
            "void_card_confirm_dlg" => corrections::void_card(ui, state, focused),
            "crossing_number_dlg" => corrections::crossing_number(ui, state, focused),
            "set_start_dlg" => corrections::set_start(ui, state, focused),
            "crossing_detail_dlg" => corrections::detail(ui, state),
            "results_dlg" => results::body(ui, state),
            "publish_wordpress_dlg" => publish::form_body(ui, state, focused),
            "publish_running_dlg" => publish::running_body(ui, state),
            "simulation_dlg" => simulation::form_body(ui, state),
            "sim_running_dlg" => simulation::running_body(ui, state),
            "resume_dlg" => message_body(ui, state.launch_message.as_deref()),
            "exit_running_dlg" => message_body(ui, state.quit_message.as_deref()),
            _ => generic_fields(ui, state, top.id, focused),
        }
        ui.separator();
        ui.horizontal(|ui| {
            for button in spec.buttons {
                let response = ui
                    .push_id(button.as_str(), |ui| {
                        ui.add_enabled(
                            button_enabled(state, top.id, *button),
                            egui::Button::new(button_label(state, top.id, *button)),
                        )
                    })
                    .inner;
                if top.focus == Focus::Button(*button) {
                    focus_ring(ui, &response);
                }
                if response.clicked() {
                    activation = Some(Activation::Button(*button));
                }
            }
        });
    });
    if modal.should_close() {
        activation = Some(Activation::Escape);
    }
    if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter)) {
        activation = Some(Activation::Enter);
    }
    apply(state, spec, activation);
}

/// Show the pending native confirmation, when an action raised one.
///
/// The action has not run yet: the confirmation queues its command only when the operator picks
/// the confirm button. Cancel, Esc and a backdrop click clear the confirmation and run nothing.
/// The Cancel button holds the keyboard focus, so Enter is the safe answer.
pub(crate) fn show_native(ctx: &Context, state: &mut AppState) {
    let Some(native) = state.pending_native.as_ref() else {
        return;
    };
    if matches!(native.then, NativeTask::ExportFile { .. }) {
        // A save picker has no in-app stand-in; an unavailable host posts the notice instead.
        return;
    }
    let dialog = native.dialog;
    let then = native.then.clone();
    let title = AppState::native_title(dialog);
    let message = state.native_message(native);
    let confirm = AppState::native_confirm_label(native);
    let mut choice: Option<bool> = None;
    let ctx = ctx.clone();
    let focus_key = egui::Id::unique(("native-confirm-focus", then.as_str()));
    let modal =
        egui::Modal::new(egui::Id::unique(("native-confirm", then.as_str()))).show(&ctx, |ui| {
            ui.set_min_width(380.0);
            ui.heading(title).accessible_name(title);
            text(ui, &message);
            ui.separator();
            ui.horizontal(|ui| {
                let cancel = ui
                    .push_id("native_cancel_btn", |ui| ui.button("Cancel"))
                    .inner;
                let accepted = ui
                    .push_id("native_confirm_btn", |ui| ui.button(confirm))
                    .inner
                    .accessible_name(format!("Confirm {confirm}"));
                let focused = ui.data(|data| data.get_temp::<bool>(focus_key).unwrap_or(false));
                if !focused {
                    cancel.request_focus();
                    ui.data_mut(|data| data.insert_temp(focus_key, true));
                }
                if cancel.clicked() {
                    choice = Some(false);
                }
                if accepted.clicked() {
                    choice = Some(true);
                }
            });
        });
    if modal.should_close() {
        choice = Some(false);
    }
    match choice {
        Some(true) => {
            let _ = state.confirm_native();
        }
        Some(false) => {
            let _ = state.cancel_native();
        }
        None => {}
    }
    if state.pending_native.is_none() {
        ctx.data_mut(|data| data.remove::<bool>(focus_key));
    }
}

/// Route the activation to the dialog stack.
fn apply(state: &mut AppState, spec: &DialogSpec, activation: Option<Activation>) {
    let Some(activation) = activation else {
        return;
    };
    let submitting = matches!(
        activation,
        Activation::Button(Button::Ok) | Activation::Enter
    );
    if submitting {
        if let Some(outcome) = state.submit_top_form() {
            // A refused form keeps the dialog open and shows its messages in the infobar.
            let _ = outcome;
            return;
        }
        if spec.id == "exit_running_dlg" {
            // OK is "Quit — keep ride running": the ride keeps timing on the wall clock.
            state.confirm_quit();
            let _ = state.dialogs.key(stack::Key::Escape);
            return;
        }
    }
    match (spec.id, activation) {
        (_, Activation::Escape) => {
            if spec.id == "simulation_dlg" {
                state.persist_sim_settings();
            }
            let _ = state.dialogs.key(stack::Key::Escape);
        }
        ("ride_library_dlg", Activation::Button(Button::Custom("duplicate_btn"))) => {
            let _ = state.duplicate_library_selected();
        }
        ("ride_library_dlg", Activation::Button(Button::Delete)) => {
            let _ = state.delete_library_selected();
        }
        ("delete_ride_dlg", Activation::Button(Button::Delete)) => {
            let _ = state.submit_delete_ride();
        }
        ("exit_running_dlg", Activation::Button(Button::Custom("finish_first_btn"))) => {
            state.finish_and_quit();
            let _ = state.dialogs.key(stack::Key::Escape);
        }
        ("resume_dlg", Activation::Button(Button::Custom("continue_btn"))) => {
            state.resume_continue();
        }
        ("resume_dlg", Activation::Button(Button::Custom("library_btn"))) => {
            state.resume_library();
        }
        ("selftest_dlg", Activation::Button(Button::Custom("rerun_btn"))) => {
            state.rerun_selftest();
        }
        ("crossing_detail_dlg", Activation::Button(Button::Custom("edit_btn"))) => {
            state.open_crossing_number();
        }
        ("crossing_detail_dlg", Activation::Button(Button::Custom("edit_time_btn"))) => {
            state.open_edit_crossing();
        }
        ("crossing_detail_dlg", Activation::Button(Button::Custom("void_card_btn"))) => {
            state.open_void_card();
        }
        ("crossing_detail_dlg", Activation::Button(Button::Custom("delete_btn"))) => {
            let _ = state.raise_delete_crossing();
        }
        ("simulation_dlg", Activation::Button(Button::Custom("gen_riders_btn"))) => {
            if let Err(message) = state.generate_sim_riders() {
                state.note(message);
            }
        }
        ("simulation_dlg", Activation::Button(Button::Custom("check_btn"))) => {
            state.check_simulation();
        }
        ("publish_running_dlg", Activation::Button(Button::Custom("cancel_btn"))) => {
            state.request_publish_cancel();
        }
        ("sim_running_dlg", Activation::Button(Button::Custom("cancel_btn"))) => {
            state.request_sim_cancel();
        }
        (_, Activation::Enter) => {
            let _ = state.dialogs.key(stack::Key::Enter);
        }
        (_, Activation::Button(button)) if button == spec.cancel || is_close(button) => {
            let _ = state.dialogs.key(stack::Key::Escape);
        }
        (_, Activation::Button(button)) => {
            let _ = state.dialogs.activate(button);
        }
    }
}

/// Return whether the button only dismisses its dialog.
fn is_close(button: Button) -> bool {
    button == Button::Close
}

/// Show a dialog's message line.
fn message_body(ui: &mut Ui, message: Option<&str>) {
    ui.push_id("message_lbl", |ui| text(ui, message.unwrap_or("")));
}

/// Return the dialog's displayed title; the Add windows retitle in edit mode.
fn dialog_title(state: &AppState, spec: &DialogSpec) -> &'static str {
    match spec.id {
        "add_rider_dlg" if state.rider_editor.editing.is_some() => "Edit Rider",
        "add_team_dlg" if state.team_editor.editing.is_some() => "Edit Team",
        "edit_crossing_dlg" if state.correction.add => "Add Crossing at Time",
        _ => spec.title,
    }
}

/// Return the human label of a dialog button.
fn button_label(state: &AppState, id: &str, button: Button) -> &'static str {
    match (id, button) {
        ("add_rider_dlg" | "settings_dlg", Button::Ok) => "Save",
        ("add_team_dlg", Button::Ok) if state.team_editor.editing.is_some() => "Save",
        ("add_team_dlg", Button::Ok) => "Add",
        ("csv_preview_dlg", Button::Ok) => "Import",
        ("ride_library_dlg", Button::Ok) => "Open",
        ("publish_wordpress_dlg", Button::Ok) => "Publish",
        ("simulation_dlg", Button::Ok) => "GO",
        ("simulation_dlg", Button::Custom("gen_riders_btn")) => "Generate Riders",
        ("simulation_dlg", Button::Custom("check_btn")) => "Check",
        ("publish_running_dlg" | "sim_running_dlg", Button::Custom("cancel_btn"))
        | (_, Button::Cancel) => "Cancel",
        ("exit_running_dlg", Button::Ok) => "Quit — keep ride running",
        (_, Button::Ok) => "OK",
        (_, Button::Close) => "Close",
        (_, Button::Delete) => "Delete",
        (_, Button::Custom("continue_btn")) => "Continue",
        (_, Button::Custom("library_btn")) => "Open Library",
        (_, Button::Custom("finish_first_btn")) => "Finish first",
        (_, Button::Custom("rerun_btn")) => "Run again",
        ("crossing_detail_dlg", Button::Custom("edit_btn")) => "Edit Plate",
        ("crossing_detail_dlg", Button::Custom("edit_time_btn")) => "Edit Time…",
        ("crossing_detail_dlg", Button::Custom("void_card_btn")) => "Void Card…",
        (_, Button::Custom(id)) => id,
    }
}

/// Return whether the dialog button accepts a click.
fn button_enabled(state: &AppState, id: &str, button: Button) -> bool {
    let actions = state.crossing_detail.as_ref().map(|detail| detail.actions);
    match (id, button) {
        ("csv_preview_dlg", Button::Ok) => state
            .csv_preview
            .model
            .as_ref()
            .is_some_and(|model| model.can_import),
        ("delete_ride_dlg", Button::Delete) => state.delete_ride_armed(),
        ("ride_library_dlg", Button::Ok | Button::Custom("duplicate_btn")) => state.library_ready(),
        ("ride_library_dlg", Button::Delete) => state.library_delete_ready(),
        ("crossing_detail_dlg", Button::Custom("edit_btn")) => {
            actions.is_some_and(|actions| actions.edit)
        }
        ("crossing_detail_dlg", Button::Custom("edit_time_btn")) => {
            actions.is_some_and(|actions| actions.edit_time)
        }
        ("crossing_detail_dlg", Button::Custom("void_card_btn")) => {
            actions.is_some_and(|actions| actions.void_card)
        }
        ("crossing_detail_dlg", Button::Custom("delete_btn")) => {
            actions.is_some_and(|actions| actions.delete)
        }
        _ => true,
    }
}

/// Show one text field bound to a dialog's frozen field id.
pub(crate) fn stack_text_field(
    ui: &mut Ui,
    state: &mut AppState,
    id: &str,
    label: &str,
    focused: Option<&str>,
) {
    let mut text = state.dialogs.field(id).unwrap_or_default().to_owned();
    let response = ui
        .push_id(id, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(label).selectable(false))
                    .accessible_name(label);
                ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .desired_width(240.0)
                        .hint_text(label),
                )
                .accessible_name(label)
            })
            .inner
        })
        .inner;
    if focused == Some(id) {
        focus_ring(ui, &response);
    }
    if response.changed() {
        let _ = state.dialogs.set_field(id, text);
    }
}

/// Show the dialog's declared fields as text fields.
fn generic_fields(ui: &mut Ui, state: &mut AppState, window: &str, focused: Option<&str>) {
    for field in ids::fields()
        .into_iter()
        .filter(|field| field.window == window)
    {
        let mut text = state.dialogs.field(field.id).unwrap_or_default().to_owned();
        let label = field_label(field.id);
        let response = ui
            .push_id(field.id, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(label.clone()).selectable(false));
                    ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .desired_width(240.0)
                            .hint_text(label.clone()),
                    )
                    .accessible_name(label)
                })
                .inner
            })
            .inner;
        if focused == Some(field.id) {
            focus_ring(ui, &response);
        }
        if response.changed() {
            let _ = state.dialogs.set_field(field.id, text);
        }
    }
    if window == "start_blocked_dlg" {
        ui.add(egui::Label::new(state.notice.as_deref().unwrap_or("")).selectable(false));
    }
}

/// Return the caption of a declared field.
fn field_label(id: &str) -> String {
    match id {
        "confirm_name_input" => "Type the ride name",
        "start_date_picker" => "Date (YYYY-MM-DD)",
        "start_time_picker" => "Start time (HH:MM)",
        "reason_input" => "Reason",
        "plate_input" => "Plate",
        other => other,
    }
    .to_owned()
}

/// Draw the focus ring around a control.
pub(crate) fn focus_ring(ui: &Ui, response: &Response) {
    ui.painter().rect_stroke(
        response.rect.expand(2.0),
        egui::CornerRadius::same(4),
        Stroke::new(2.0, ui.visuals().selection.stroke.color),
        egui::StrokeKind::Outside,
    );
}
