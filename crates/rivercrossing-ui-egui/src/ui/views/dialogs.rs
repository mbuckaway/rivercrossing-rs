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
use super::{csv, issues, riders, setup, teams};
use crate::ui::action::spec::NativeDialog;
use crate::ui::dialogs::spec::{self, Button, DialogSpec};
use crate::ui::dialogs::stack::{self, Focus};
use crate::ui::ids;
use crate::ui::presenters::riders::delete_message;
use crate::ui::presenters::teams::remove_message;
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
    let dialog = native.dialog;
    let then = native.then.clone();
    let title = native_title(dialog);
    let message = native_message(state, &then);
    let confirm = native_confirm_label(&then);
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

/// Return the native confirmation's title for the dialog kind.
fn native_title(dialog: NativeDialog) -> &'static str {
    match dialog {
        NativeDialog::Confirm => "Confirm",
        NativeDialog::Danger => "Warning",
        NativeDialog::Prompt => "Input required",
        NativeDialog::Error => "Error",
    }
}

/// Return the confirmation's message, naming the action or record it guards.
fn native_message(state: &AppState, task: &NativeTask) -> String {
    use crate::ui::action::spec::CommandId;
    match task {
        NativeTask::Command(CommandId::UndoCrossing) => "Undo the last crossing?".to_owned(),
        NativeTask::Command(CommandId::StopRide) => "Stop the ride?".to_owned(),
        NativeTask::Command(CommandId::FinishRide) => "Finish the ride?".to_owned(),
        NativeTask::Command(CommandId::ClearRide) => {
            "Clear the ride from the console? The database is untouched.".to_owned()
        }
        NativeTask::Command(CommandId::ReopenRide) => {
            "Reopen the finished ride for corrections?".to_owned()
        }
        NativeTask::Command(CommandId::DuplicateRide) => "Duplicate the open ride?".to_owned(),
        NativeTask::Command(CommandId::AppQuit) => "Quit RiverCrossing?".to_owned(),
        NativeTask::Command(other) => format!("Run {}?", other.as_str()),
        NativeTask::DeleteRider { entry, .. } => state
            .ride
            .as_ref()
            .and_then(|ride| ride.roster.entry_by_key(&entry.0))
            .map_or_else(|| "Delete the selected rider?".to_owned(), delete_message),
        NativeTask::RemoveTeam { entry } => state
            .ride
            .as_ref()
            .and_then(|ride| ride.roster.entry_by_key(&entry.0))
            .map_or_else(|| "Remove the selected team?".to_owned(), remove_message),
    }
}

/// Return the confirm button's visible label.
fn native_confirm_label(task: &NativeTask) -> &'static str {
    use crate::ui::action::spec::CommandId;
    match task {
        NativeTask::Command(CommandId::UndoCrossing) => "Undo",
        NativeTask::Command(CommandId::StopRide) => "Stop",
        NativeTask::Command(CommandId::FinishRide) => "Finish",
        NativeTask::Command(CommandId::ClearRide) => "Clear",
        NativeTask::Command(CommandId::ReopenRide) => "Reopen",
        NativeTask::Command(CommandId::DuplicateRide) => "Duplicate",
        NativeTask::Command(CommandId::AppQuit) => "Quit",
        NativeTask::Command(_) => "OK",
        NativeTask::DeleteRider { .. } => "Delete",
        NativeTask::RemoveTeam { .. } => "Remove",
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
    if submitting && let Some(outcome) = state.submit_top_form() {
        // A refused form keeps the dialog open and shows its messages in the infobar.
        let _ = outcome;
        return;
    }
    match activation {
        Activation::Escape => {
            let _ = state.dialogs.key(stack::Key::Escape);
        }
        Activation::Enter => {
            let _ = state.dialogs.key(stack::Key::Enter);
        }
        Activation::Button(button) if button == spec.cancel || is_close(button) => {
            let _ = state.dialogs.key(stack::Key::Escape);
        }
        Activation::Button(button) => {
            let _ = state.dialogs.activate(button);
        }
    }
}

/// Return whether the button only dismisses its dialog.
fn is_close(button: Button) -> bool {
    button == Button::Close
}

/// Return the dialog's displayed title; the Add windows retitle in edit mode.
fn dialog_title(state: &AppState, spec: &DialogSpec) -> &'static str {
    match spec.id {
        "add_rider_dlg" if state.rider_editor.editing.is_some() => "Edit Rider",
        "add_team_dlg" if state.team_editor.editing.is_some() => "Edit Team",
        _ => spec.title,
    }
}

/// Return the human label of a dialog button.
fn button_label(state: &AppState, id: &str, button: Button) -> &'static str {
    match (id, button) {
        ("add_rider_dlg", Button::Ok) => "Save",
        ("add_team_dlg", Button::Ok) if state.team_editor.editing.is_some() => "Save",
        ("add_team_dlg", Button::Ok) => "Add",
        ("csv_preview_dlg", Button::Ok) => "Import",
        (_, Button::Ok) => "OK",
        (_, Button::Cancel) => "Cancel",
        (_, Button::Close) => "Close",
        (_, Button::Delete) => "Delete",
        (_, Button::Custom("continue_btn")) => "Continue",
        (_, Button::Custom("library_btn")) => "Ride Library",
        (_, Button::Custom("finish_first_btn")) => "Finish first",
        (_, Button::Custom(id)) => id,
    }
}

/// Return whether the dialog button accepts a click.
fn button_enabled(state: &AppState, id: &str, button: Button) -> bool {
    match (id, button) {
        ("csv_preview_dlg", Button::Ok) => state
            .csv_preview
            .model
            .as_ref()
            .is_some_and(|model| model.can_import),
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
