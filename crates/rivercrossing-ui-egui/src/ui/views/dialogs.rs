// SPDX-License-Identifier: GPL-3.0-only

//! The dialog host: the top [`DialogStack`] dialog shown as an `ArtCraft` `dialog::Dialog`.
//!
//! The title, the fields and the buttons in the frozen contract's order come from
//! [`crate::ui::dialogs::spec`]. The craft dialog owns Esc, Enter, the focus trap and the footer's
//! platform order; a clicked or activated button reports its frozen button through
//! [`DialogStack::activate`] and Esc through [`DialogStack::key`]. The ride setup's OK validates its
//! form and hands the config to the app; every other dialog only reports its event until its own
//! screen lands in a later release.

use std::borrow::Cow;

use craft::behavior::ColumnKey;
use craft::behavior::{ColumnWidth, DisplayOrder, ItemSource, Selection};
use craft::egui::Ui;
use craft::theme::variant::Tone;
use craft::ui::choice::{Checkbox, Radio, RadioGroup};
use craft::ui::contract::{
    Bind, ColumnSpec, Entry, LeastWidth, Props as _, Show as _, ShowCollection as _,
    ShowContainer as _, Sizing,
};
use craft::ui::dialog::Dialog;
use craft::ui::display::Text;
use craft::ui::list::{List, ListRow};
use craft::ui::notices::Notice;
use craft::ui::pickers::{DatePicker, PickedDate, Select};
use craft::ui::runtime::overlay::CloseReason;
use craft::ui::sections::{DialogRole, FooterButton};
use craft::ui::table::Table;
use craft::ui::text::{NumberField, TextField};

use super::{
    audit, corrections, csv, issues, library, publish, results, riders, selftest, settings, setup,
    simulation, system, teams,
};
use crate::ui::dialogs::spec::{self, Button, DialogSpec};
use crate::ui::dialogs::stack::{self, Key};
use crate::ui::state::{AppState, NativeTask};

#[cfg(test)]
mod tests;

/// The dialogs whose tables need a wide window.
const WIDE: [&str; 8] = [
    "ride_setup_dlg",
    "rider_editor_dlg",
    "csv_preview_dlg",
    "team_editor_dlg",
    "rider_issues_dlg",
    "ride_library_dlg",
    "audit_dlg",
    "results_dlg",
];

/// The name column, shared by the dialogs whose first column names a row.
pub(crate) const NAME: ColumnKey = ColumnKey::new("name");
/// The plate column.
pub(crate) const PLATE: ColumnKey = ColumnKey::new("plate");
/// The team column.
pub(crate) const TEAM: ColumnKey = ColumnKey::new("team");
/// The lap column.
pub(crate) const LAP: ColumnKey = ColumnKey::new("lap");
/// The time column.
pub(crate) const TIME: ColumnKey = ColumnKey::new("time");
/// The date column.
pub(crate) const DATE: ColumnKey = ColumnKey::new("date");
/// The kind column.
pub(crate) const KIND: ColumnKey = ColumnKey::new("kind");
/// The count column.
pub(crate) const COUNT: ColumnKey = ColumnKey::new("count");
/// The status column.
pub(crate) const STATUS: ColumnKey = ColumnKey::new("status");
/// The issue column.
pub(crate) const ISSUE: ColumnKey = ColumnKey::new("issue");
/// The action column.
pub(crate) const ACTION: ColumnKey = ColumnKey::new("action");
/// The entry column.
pub(crate) const ENTRY: ColumnKey = ColumnKey::new("entry");
/// The reason column.
pub(crate) const REASON: ColumnKey = ColumnKey::new("reason");
/// The shortcut column.
pub(crate) const SHORTCUT: ColumnKey = ColumnKey::new("shortcut");
/// The place column.
pub(crate) const PLACE: ColumnKey = ColumnKey::new("place");
/// The best-hand column.
pub(crate) const HAND: ColumnKey = ColumnKey::new("hand");
/// The total-time column.
pub(crate) const TOTAL: ColumnKey = ColumnKey::new("total");

/// What closed the dialog this pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Activation {
    /// A clicked or activated button.
    Button(Button),
    /// Esc, the close action, or another close that is not a pick.
    Escape,
}

/// Show the top dialog, when one is open.
pub(crate) fn show(ui: &mut Ui, state: &mut AppState) {
    let Some(top) = state.dialogs.top().cloned() else {
        return;
    };
    let Some(spec) = spec::spec(top.id) else {
        return;
    };
    let mut open = true;
    let output = ui
        .push_id(spec.id, |ui| {
            let title = dialog_title(state, spec);
            let shown = Dialog::new(title)
                .least_width(least_width(spec))
                .buttons(footer_buttons(state, spec))
                .close_on_enter(true)
                .opened(Bind::bound(&mut open))
                .show(ui, |ui, _| body(ui, state, spec.id));
            shown.output
        })
        .inner;
    apply(state, spec, activation(output.closed, output.picked, spec));
}

/// Show the pending native confirmation, when an action raised one.
///
/// The action has not run yet: the confirmation queues its command only when the operator picks
/// the confirm button. Cancel, Esc and a backdrop click clear the confirmation and run nothing.
/// Cancel is the accepting button, so it holds the keyboard focus and Enter is the safe answer.
pub(crate) fn show_native(ui: &mut Ui, state: &mut AppState) {
    let Some(native) = state.pending_native.as_ref() else {
        return;
    };
    if matches!(native.then, NativeTask::ExportFile { .. }) {
        // A save picker has no in-app stand-in; an unavailable host posts the notice instead.
        return;
    }
    let title = AppState::native_title(native.dialog);
    let message = state.native_message(native);
    let confirm = format!("Confirm {}", AppState::native_confirm_label(native));
    let then = native.then.clone();
    let mut open = true;
    let output = ui
        .push_id(("native-confirm", then.as_str()), |ui| {
            let buttons = [
                FooterButton::new("Cancel", DialogRole::Accept),
                FooterButton::new(confirm, DialogRole::Destructive),
            ];
            Dialog::new(title)
                .buttons(buttons)
                .opened(Bind::bound(&mut open))
                .show(ui, |ui, _| {
                    let _ = Text::new(message.clone()).show(ui);
                })
                .output
        })
        .inner;
    if output.picked == Some(1) {
        let _ = state.confirm_native();
    } else if output.picked.is_some() || output.closed.is_some() {
        let _ = state.cancel_native();
    }
}

/// Return the button a footer pick or the dialog's close reports.
fn activation(
    closed: Option<CloseReason>,
    picked: Option<usize>,
    spec: &DialogSpec,
) -> Option<Activation> {
    if let Some(index) = picked {
        return spec.buttons.get(index).copied().map(Activation::Button);
    }
    match closed? {
        CloseReason::Enter | CloseReason::Activated => {
            Some(Activation::Button(stack::primary_button(spec)))
        }
        _ => Some(Activation::Escape),
    }
}

/// Return the footer role of *button*: the Enter button accepts, the cancel leaves, and the
/// destructive outcomes take the danger look.
fn button_role(spec: &DialogSpec, button: Button) -> DialogRole {
    if button == stack::primary_button(spec) {
        return DialogRole::Accept;
    }
    if button == spec.cancel {
        return DialogRole::Cancel;
    }
    if is_danger(button) {
        return DialogRole::Destructive;
    }
    DialogRole::Other
}

/// Return whether the button destroys something. An OK that is neither the default nor the cancel
/// is the Exit Running dialog's "quit and keep the ride running", the one destructive OK.
fn is_danger(button: Button) -> bool {
    matches!(
        button,
        Button::Ok | Button::Delete | Button::Custom("delete_btn" | "void_card_btn")
    )
}

/// Return the dialog's least width: the wide role for the dialogs whose tables need it.
fn least_width(spec: &DialogSpec) -> LeastWidth {
    if WIDE.contains(&spec.id) {
        LeastWidth::WideDialog
    } else {
        LeastWidth::Dialog
    }
}

/// Return the footer buttons: the frozen labels, roles and enabled states.
fn footer_buttons(state: &AppState, spec: &DialogSpec) -> Vec<FooterButton> {
    spec.buttons
        .iter()
        .map(|button| {
            FooterButton::new(
                button_label(state, spec.id, *button),
                button_role(spec, *button),
            )
            .enabled(button_enabled(state, spec.id, *button))
        })
        .collect()
}

/// Show the dialog's body, dispatched by its frozen window id.
fn body(ui: &mut Ui, state: &mut AppState, id: &str) {
    match id {
        "ride_setup_dlg" => setup::body(ui, state),
        "add_rider_dlg" => riders::form_body(ui, state),
        "add_team_dlg" => teams::form_body(ui, state),
        "edit_crossing_dlg" => corrections::edit_crossing(ui, state),
        "manual_deal_dlg" => corrections::manual_deal(ui, state),
        "dnf_confirm_dlg" => corrections::dnf(ui, state),
        "void_card_confirm_dlg" => corrections::void_card(ui, state),
        "crossing_number_dlg" => corrections::crossing_number(ui, state),
        "set_start_dlg" => corrections::set_start(ui, state),
        "publish_wordpress_dlg" => publish::form_body(ui, state),
        "simulation_dlg" => simulation::form_body(ui, state),
        other => report_body(ui, state, other),
    }
}

/// Show a dialog whose body is a report or a table.
fn report_body(ui: &mut Ui, state: &mut AppState, id: &str) {
    match id {
        "rider_editor_dlg" => riders::editor_body(ui, state),
        "team_editor_dlg" => teams::editor_body(ui, state),
        "csv_preview_dlg" => csv::body(ui, state),
        "rider_issues_dlg" => issues::body(ui, state),
        "ride_library_dlg" => library::body(ui, state),
        "audit_dlg" => audit::body(ui, state),
        "results_dlg" => results::body(ui, state),
        "shortcuts_dlg" => system::shortcuts(ui),
        "selftest_dlg" => selftest::body(ui, state),
        "crossing_detail_dlg" => corrections::detail(ui, state),
        other => notice_body(ui, state, other),
    }
}

/// Show a dialog whose body is a message, a notice or a progress window.
fn notice_body(ui: &mut Ui, state: &mut AppState, id: &str) {
    match id {
        "delete_ride_dlg" => library::delete_body(ui, state),
        "resume_dlg" => message_body(ui, state.launch_message.clone()),
        "exit_running_dlg" => message_body(ui, state.quit_message.clone()),
        "start_blocked_dlg" => blocked_body(ui, state),
        "settings_dlg" => settings::body(ui, state),
        "about_dlg" => system::about(ui),
        "publish_running_dlg" => publish::running_body(ui, state),
        "sim_running_dlg" => simulation::running_body(ui, state),
        _ => {}
    }
}

/// Show a dialog's message line.
fn message_body(ui: &mut Ui, message: Option<String>) {
    let shown = message.unwrap_or_default();
    ui.push_id("message_lbl", |ui| {
        let _ = Text::new(shown).show(ui);
    });
}

/// Show why the ride cannot start: the notice over the frozen reason list.
fn blocked_body(ui: &mut Ui, state: &AppState) {
    let reasons: Vec<String> = state
        .notice
        .clone()
        .unwrap_or_default()
        .split("; ")
        .map(str::to_owned)
        .collect();
    let _ = Notice::new(Tone::Bad, "The ride cannot start yet.").show(ui);
    ui.push_id("start_blocked_list", |ui| {
        let _ = List::<usize>::new("Cannot start").max_rows(6).show_items(
            ui,
            &Lines(&reasons),
            &mut |key, _| ListRow::new(reasons.get(*key).cloned().unwrap_or_default()),
        );
    });
}

/// Route the activation to the dialog stack.
fn apply(state: &mut AppState, spec: &DialogSpec, activation: Option<Activation>) {
    let Some(activation) = activation else {
        return;
    };
    let Activation::Button(button) = activation else {
        escape(state, spec);
        return;
    };
    if button == Button::Ok && !spec.destructive && state.submit_top_form().is_some() {
        // A refused form keeps the dialog open and shows its messages in the infobar; a submitted
        // one closes itself.
        return;
    }
    route(state, spec, button);
}

/// Route one button to its dialog's action, or close the dialog.
fn route(state: &mut AppState, spec: &DialogSpec, button: Button) {
    if spec.id == "exit_running_dlg" && button == Button::Ok {
        // OK is "Quit — keep ride running": the ride keeps timing on the wall clock.
        state.confirm_quit();
        escape(state, spec);
        return;
    }
    if route_editor(state, spec.id, button) || route_running(state, spec.id, button) {
        return;
    }
    if button == spec.cancel || is_close(button) {
        escape(state, spec);
        return;
    }
    let _ = state.dialogs.activate(button);
}

/// Run an editor or list dialog's own action for *button*; return whether it had one.
fn route_editor(state: &mut AppState, id: &str, button: Button) -> bool {
    match (id, button) {
        ("ride_library_dlg", Button::Custom("duplicate_btn")) => {
            let _ = state.duplicate_library_selected();
        }
        ("ride_library_dlg", Button::Delete) => {
            let _ = state.delete_library_selected();
        }
        ("delete_ride_dlg", Button::Delete) => {
            let _ = state.submit_delete_ride();
        }
        ("resume_dlg", Button::Custom("continue_btn")) => state.resume_continue(),
        ("resume_dlg", Button::Custom("library_btn")) => state.resume_library(),
        ("selftest_dlg", Button::Custom("rerun_btn")) => state.rerun_selftest(),
        ("crossing_detail_dlg", Button::Custom("edit_btn")) => state.open_crossing_number(),
        ("crossing_detail_dlg", Button::Custom("edit_time_btn")) => state.open_edit_crossing(),
        ("crossing_detail_dlg", Button::Custom("void_card_btn")) => state.open_void_card(),
        ("crossing_detail_dlg", Button::Custom("delete_btn")) => {
            let _ = state.raise_delete_crossing();
        }
        _ => return false,
    }
    true
}

/// Run a lifecycle, simulator or publish dialog's own action for *button*; return whether it had
/// one.
fn route_running(state: &mut AppState, id: &str, button: Button) -> bool {
    match (id, button) {
        ("exit_running_dlg", Button::Custom("finish_first_btn")) => state.finish_and_quit(),
        ("simulation_dlg", Button::Custom("new_ride_btn")) => {
            let today = time::OffsetDateTime::now_utc().date();
            if let Err(message) = state.create_sim_test_ride(today) {
                state.note(message);
            }
        }
        ("simulation_dlg", Button::Custom("gen_riders_btn")) => {
            if let Err(message) = state.generate_sim_riders() {
                state.note(message);
            }
        }
        ("simulation_dlg", Button::Custom("check_btn")) => state.check_simulation(),
        ("simulation_dlg", Button::Custom("go_btn")) => {
            if let Err(message) = state.submit_simulation() {
                state.note(message);
            }
        }
        ("publish_running_dlg", Button::Custom("cancel_btn")) => state.request_publish_cancel(),
        ("sim_running_dlg", Button::Custom("cancel_btn")) => state.request_sim_cancel(),
        _ => return false,
    }
    true
}

/// Close the dialog as its cancel: Esc, the close action, or the safe button.
fn escape(state: &mut AppState, spec: &DialogSpec) {
    if spec.id == "simulation_dlg" {
        state.persist_sim_settings();
    }
    let _ = state.dialogs.key(Key::Escape);
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
        ("simulation_dlg", Button::Custom("new_ride_btn")) => "New Ride",
        ("simulation_dlg", Button::Custom("gen_riders_btn")) => "Generate Riders",
        ("simulation_dlg", Button::Custom("check_btn")) => "Check",
        ("simulation_dlg", Button::Custom("go_btn")) => "GO",
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
        ("simulation_dlg", Button::Custom("new_ride_btn")) => state.sim_new_ride_enabled(),
        ("simulation_dlg", Button::Custom("go_btn")) => state.sim_go_enabled(),
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

// --------------------------------------------------------------------- fields

/// Show one text field bound to a dialog's frozen field id.
pub(crate) fn stack_text_field(
    ui: &mut Ui,
    state: &mut AppState,
    id: &'static str,
    label: &'static str,
) {
    let stored = state.dialogs.field(id).unwrap_or_default().to_owned();
    let mut value = stored.clone();
    ui.push_id(id, |ui| {
        let _ = TextField::new(label, Bind::bound(&mut value)).show(ui);
    });
    if value != stored {
        let _ = state.dialogs.set_field(id, value);
    }
}

/// Show one masked text field bound to a dialog's frozen field id.
pub(crate) fn stack_secret_field(
    ui: &mut Ui,
    state: &mut AppState,
    id: &'static str,
    label: &'static str,
) {
    let stored = state.dialogs.field(id).unwrap_or_default().to_owned();
    let mut value = stored.clone();
    ui.push_id(id, |ui| {
        let _ = TextField::new(label, Bind::bound(&mut value))
            .masked()
            .show(ui);
    });
    if value != stored {
        let _ = state.dialogs.set_field(id, value);
    }
}

/// Show one date picker bound to a dialog's ISO-date field.
pub(crate) fn stack_date_field(
    ui: &mut Ui,
    state: &mut AppState,
    id: &'static str,
    label: &'static str,
) {
    let stored = state.dialogs.field(id).unwrap_or_default().to_owned();
    let mut picked = PickedDate::parse(&stored);
    ui.push_id(id, |ui| {
        let _ = DatePicker::new(label, Bind::bound(&mut picked)).show(ui);
    });
    let text = picked.map_or_else(String::new, |date| date.iso());
    if text != stored {
        let _ = state.dialogs.set_field(id, text);
    }
}

/// Show one whole-number field bound to a dialog's field, written back as a plain integer.
pub(crate) fn stack_whole_field(
    ui: &mut Ui,
    state: &mut AppState,
    id: &'static str,
    label: &'static str,
    default: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    let stored = state.dialogs.field(id).unwrap_or_default().to_owned();
    let mut value = Some(stored.trim().parse().unwrap_or(default));
    ui.push_id(id, |ui| {
        let _ = NumberField::new(label, Bind::bound(&mut value))
            .range(range)
            .step(1.0)
            .snap_typed(true)
            .show(ui);
    });
    let text = value.map_or_else(String::new, |number| format!("{number:.0}"));
    if text != stored {
        let _ = state.dialogs.set_field(id, text);
    }
}

/// Show one decimal field bound to a dialog's field, written back with one decimal.
pub(crate) fn stack_decimal_field(
    ui: &mut Ui,
    state: &mut AppState,
    id: &'static str,
    label: &'static str,
    default: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    let stored = state.dialogs.field(id).unwrap_or_default().to_owned();
    let mut value = Some(stored.trim().parse().unwrap_or(default));
    ui.push_id(id, |ui| {
        let _ = NumberField::new(label, Bind::bound(&mut value))
            .range(range)
            .step(0.1)
            .show(ui);
    });
    let text = value.map_or_else(String::new, |number| format!("{number:.1}"));
    if text != stored {
        let _ = state.dialogs.set_field(id, text);
    }
}

/// Show one decimal field over a value the caller holds, written back with one decimal.
pub(crate) fn decimal_value(
    ui: &mut Ui,
    id: &'static str,
    label: &'static str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
) {
    let mut number = Some(*value);
    ui.push_id(id, |ui| {
        let _ = NumberField::new(label, Bind::bound(&mut number))
            .range(range)
            .step(0.1)
            .show(ui);
    });
    if let Some(number) = number {
        *value = number;
    }
}

/// Show one select bound to a dialog's field; *choices* pairs each visible label with its stored
/// value, and the first entry shows while the stored value names none of them.
pub(crate) fn stack_select(
    ui: &mut Ui,
    state: &mut AppState,
    id: &'static str,
    label: &'static str,
    choices: &[(&'static str, &'static str)],
) {
    let stored = state.dialogs.field(id).unwrap_or_default().to_owned();
    if let Some(value) = select_value(ui, id, label, &stored, choices) {
        let _ = state.dialogs.set_field(id, value);
    }
}

/// Show one select over *choices*; return the stored value picked this pass, when it changed.
pub(crate) fn select_value(
    ui: &mut Ui,
    id: &'static str,
    label: &'static str,
    stored: &str,
    choices: &[(&'static str, &'static str)],
) -> Option<&'static str> {
    let names: Vec<String> = choices.iter().map(|(name, _)| (*name).to_owned()).collect();
    let current = choices
        .iter()
        .position(|(_, value)| *value == stored)
        .unwrap_or(0);
    let picked = pick(ui, id, label, &names, current);
    if picked == current {
        return None;
    }
    choices.get(picked).map(|(_, value)| *value)
}

/// Show one select over choices built at run time; return the value picked this pass, when it
/// changed.
pub(crate) fn select_owned(
    ui: &mut Ui,
    id: &'static str,
    label: &'static str,
    stored: &str,
    choices: &[(String, String)],
) -> Option<String> {
    let names: Vec<String> = choices.iter().map(|(name, _)| name.clone()).collect();
    let current = choices
        .iter()
        .position(|(_, value)| *value == stored)
        .unwrap_or(0);
    let picked = pick(ui, id, label, &names, current);
    if picked == current {
        return None;
    }
    choices.get(picked).map(|(_, value)| value.clone())
}

/// Show a select of *names* with *current* picked; return the picked index.
fn pick(
    ui: &mut Ui,
    id: &'static str,
    label: &'static str,
    names: &[String],
    current: usize,
) -> usize {
    let mut picked = current;
    let entries: Vec<Entry<usize>> = names
        .iter()
        .enumerate()
        .map(|(at, name)| Entry::new(at, name.clone()))
        .collect();
    ui.push_id(id, |ui| {
        let _ = Select::new(label, Bind::bound(&mut picked), &entries)
            .sizing(Sizing::Content)
            .show(ui);
    });
    picked
}

/// Show a check box bound to a dialog's `"true"`/`"false"` field; return the new value when it
/// changed.
pub(crate) fn stack_flag(
    ui: &mut Ui,
    state: &AppState,
    id: &'static str,
    label: &'static str,
) -> Option<bool> {
    let stored = state.dialogs.field(id) == Some("true");
    let mut on = stored;
    ui.push_id(id, |ui| {
        let _ = Checkbox::new(Bind::bound(&mut on)).label(label).show(ui);
    });
    (on != stored).then_some(on)
}

/// Show a radio group over its frozen `_radio` controls; *options* pairs each value with its frozen
/// id and its label. Return the value picked this pass.
pub(crate) fn radio_group(
    ui: &mut Ui,
    group: &'static str,
    picked: u8,
    options: &[(u8, &'static str, &'static str)],
) -> u8 {
    let mut chosen = picked;
    ui.push_id(group, |ui| {
        let _ = RadioGroup::new(group, Bind::bound(&mut chosen)).show(ui, |ui, _| {
            for (value, name, label) in options {
                ui.push_id(*name, |ui| {
                    let _ = Radio::new(*value).label(*label).show(ui);
                });
            }
        });
    });
    chosen
}

/// Show a notice under a frozen id.
pub(crate) fn notice(ui: &mut Ui, id: &'static str, tone: Tone, message: impl Into<String>) {
    ui.push_id(id, |ui| {
        let _ = Notice::new(tone, message.into()).show(ui);
    });
}

/// Show a read-only value under a frozen id, after its caption.
pub(crate) fn value(
    ui: &mut Ui,
    id: &'static str,
    caption: &'static str,
    value: impl Into<String>,
) {
    ui.push_id(id, |ui| {
        ui.vertical(|ui| {
            let _ = Text::new(caption).show(ui);
            let _ = Text::new(value.into()).show(ui);
        });
    });
}

// --------------------------------------------------------------------- tables

/// One row of a dialog table: its accessible label and its cells.
pub(crate) struct TableRow {
    /// The row's label, which also names it to assistive technology.
    pub(crate) label: String,
    /// The row's cells, by column.
    pub(crate) cells: Vec<(ColumnKey, String)>,
}

/// A dialog table's columns and rows.
pub(crate) struct TableData {
    /// The table's columns.
    pub(crate) columns: Vec<ColumnSpec>,
    /// The table's rows.
    pub(crate) rows: Vec<TableRow>,
}

impl TableData {
    /// Return the label of the row at *key*.
    fn label(&self, key: usize) -> String {
        self.rows
            .get(key)
            .map_or_else(String::new, |row| row.label.clone())
    }
}

/// The table's rows as a keyed collection source.
struct Rows<'a>(&'a TableData);

impl DisplayOrder<usize> for Rows<'_> {
    fn len(&self) -> usize {
        self.0.rows.len()
    }

    fn key(&self, at: usize) -> usize {
        at
    }

    fn index_of(&self, key: &usize) -> Option<usize> {
        (*key < self.0.rows.len()).then_some(*key)
    }

    fn text(&self, at: usize) -> Option<Cow<'_, str>> {
        self.0
            .rows
            .get(at)
            .map(|row| Cow::Borrowed(row.label.as_str()))
    }
}

impl ItemSource<usize> for Rows<'_> {}

/// A list of plain lines as a keyed collection source.
struct Lines<'a>(&'a [String]);

impl DisplayOrder<usize> for Lines<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn key(&self, at: usize) -> usize {
        at
    }

    fn index_of(&self, key: &usize) -> Option<usize> {
        (*key < self.0.len()).then_some(*key)
    }

    fn text(&self, at: usize) -> Option<Cow<'_, str>> {
        self.0.get(at).map(|line| Cow::Borrowed(line.as_str()))
    }
}

impl ItemSource<usize> for Lines<'_> {}

/// Show one dialog table; return the row picked this pass, when it differs from *selected*.
pub(crate) fn table(
    ui: &mut Ui,
    name: &'static str,
    data: &TableData,
    visible: u16,
    empty: &'static str,
    selected: Option<usize>,
) -> Option<usize> {
    let mut selection = selected.map_or_else(Selection::default, Selection::single);
    ui.push_id(name, |ui| {
        let _ = Table::<usize>::new(name, data.columns.clone())
            .striped(true)
            .max_rows(visible)
            .empty(empty)
            .selection(Bind::bound(&mut selection))
            .show_items(ui, &Rows(data), &mut |key, icx| {
                let mut row = ListRow::new(data.label(*key))
                    .cells(icx.cells())
                    .striped(icx.row.striped);
                if let Some(entry) = data.rows.get(*key) {
                    for (column, text) in &entry.cells {
                        row = row.cell(column.clone(), text.clone());
                    }
                }
                row
            });
    });
    if selection.lead == selected {
        None
    } else {
        selection.lead
    }
}

/// Return the columns of a table whose cells hold plain content widths.
pub(crate) fn columns(specs: &[(ColumnKey, &'static str)]) -> Vec<ColumnSpec> {
    specs
        .iter()
        .map(|(key, header)| ColumnSpec::new(key.clone(), *header, ColumnWidth::Content))
        .collect()
}
