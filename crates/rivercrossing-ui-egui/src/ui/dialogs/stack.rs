// SPDX-License-Identifier: GPL-3.0-only

//! The dialog stack: open, close, Esc, Enter, Tab and focus return. Phase 6 renders it; this module
//! holds no egui.

use std::collections::BTreeMap;

use super::spec::{self, Button, DialogSpec};
use crate::ui::ids;

/// The control that holds keyboard focus inside one dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Focus {
    /// A form field, by its frozen control id.
    Field(&'static str),
    /// A button.
    Button(Button),
}

/// One open dialog and its per-open state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OpenDialog {
    /// The frozen window id.
    pub(crate) id: &'static str,
    /// The frozen id to refocus when the dialog closes.
    pub(crate) opener: Option<&'static str>,
    /// The typed values, by frozen field id.
    pub(crate) fields: BTreeMap<String, String>,
    /// The control that holds focus.
    pub(crate) focus: Focus,
    /// The button Enter activated, when the dialog closes by confirm.
    pub(crate) confirmed_button: Option<Button>,
}

/// One key routed to the top dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    /// Cancel the top dialog.
    Escape,
    /// Activate the top dialog's default button.
    Enter,
    /// Move focus forward inside the top dialog.
    Tab,
    /// Move focus backward inside the top dialog.
    ShiftTab,
}

/// What a key did to the stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DialogEvent {
    /// The top dialog cancelled and closed.
    Cancelled {
        /// The frozen window id that closed.
        id: &'static str,
        /// The frozen id to refocus, when the dialog recorded one.
        opener: Option<&'static str>,
    },
    /// The top dialog confirmed and closed.
    Confirmed {
        /// The frozen window id that closed.
        id: &'static str,
        /// The button the dialog activated.
        button: Button,
        /// The frozen id to refocus, when the dialog recorded one.
        opener: Option<&'static str>,
    },
    /// Tab or Shift+Tab moved focus inside the top dialog.
    FocusChanged,
}

/// Why the stack refused a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DialogError {
    /// The id is not a frozen dialog.
    UnknownDialog,
    /// The dialog does not declare the field as a settable control.
    UnknownField,
    /// No dialog is open.
    NotOpen,
}

/// The open dialogs, the top one active.
#[derive(Debug, Default)]
pub(crate) struct DialogStack {
    /// The open dialogs in opening order; the last entry is the top dialog.
    open: Vec<OpenDialog>,
}

impl DialogStack {
    /// Open the dialog on top of the stack and focus its first control.
    ///
    /// # Errors
    ///
    /// Returns `UnknownDialog` when the id is not a frozen dialog.
    pub(crate) fn open(
        &mut self,
        id: &'static str,
        opener: Option<&'static str>,
    ) -> Result<(), DialogError> {
        let spec = spec_of(id)?;
        self.open.push(OpenDialog {
            id,
            opener,
            fields: BTreeMap::new(),
            focus: initial_focus(spec),
            confirmed_button: None,
        });
        Ok(())
    }

    /// Cancel the top dialog, popping it and reporting its opener.
    ///
    /// # Errors
    ///
    /// Returns `NotOpen` when no dialog is open.
    pub(crate) fn cancel(&mut self) -> Result<DialogEvent, DialogError> {
        let dialog = self.pop()?;
        Ok(DialogEvent::Cancelled {
            id: dialog.id,
            opener: dialog.opener,
        })
    }

    /// Activate the top dialog's default button, popping it and reporting the button.
    ///
    /// A destructive dialog defaults to its cancel button, so Enter never runs the destructive
    /// action.
    ///
    /// # Errors
    ///
    /// Returns `NotOpen` when no dialog is open.
    pub(crate) fn confirm(&mut self) -> Result<DialogEvent, DialogError> {
        let mut dialog = self.pop()?;
        let spec = spec_of(dialog.id)?;
        let button = primary_button(spec);
        dialog.confirmed_button = Some(button);
        Ok(DialogEvent::Confirmed {
            id: dialog.id,
            button,
            opener: dialog.opener,
        })
    }

    /// Route one key to the top dialog.
    ///
    /// # Errors
    ///
    /// Returns `NotOpen` when no dialog is open.
    pub(crate) fn key(&mut self, key: Key) -> Result<Option<DialogEvent>, DialogError> {
        match key {
            Key::Escape => self.cancel().map(Some),
            Key::Enter => self.confirm().map(Some),
            Key::Tab => self.cycle_focus(true),
            Key::ShiftTab => self.cycle_focus(false),
        }
    }

    /// Set a field on the top dialog.
    ///
    /// # Errors
    ///
    /// Returns `NotOpen` when no dialog is open, and `UnknownField` when the top dialog does not
    /// declare the field as a settable control of its frozen window.
    pub(crate) fn set_field(
        &mut self,
        field: &str,
        value: impl Into<String>,
    ) -> Result<(), DialogError> {
        let Some(top) = self.open.last_mut() else {
            return Err(DialogError::NotOpen);
        };
        if !knows_field(top.id, field) {
            return Err(DialogError::UnknownField);
        }
        top.fields.insert(field.to_owned(), value.into());
        Ok(())
    }

    /// Return the value typed in the top dialog's field, or `None` when it is unset or no dialog is
    /// open.
    pub(crate) fn field(&self, field: &str) -> Option<&str> {
        self.top()?.fields.get(field).map(String::as_str)
    }

    /// Return the top dialog.
    pub(crate) fn top(&self) -> Option<&OpenDialog> {
        self.open.last()
    }

    /// Return true when no dialog is open.
    pub(crate) fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// Pop the top dialog.
    fn pop(&mut self) -> Result<OpenDialog, DialogError> {
        self.open.pop().ok_or(DialogError::NotOpen)
    }

    /// Move the top dialog's focus one step around its ring.
    fn cycle_focus(&mut self, forward: bool) -> Result<Option<DialogEvent>, DialogError> {
        let Some(top) = self.open.last_mut() else {
            return Err(DialogError::NotOpen);
        };
        let spec = spec_of(top.id)?;
        let current = top.focus.clone();
        let ring = focusables(spec);
        let index = ring.iter().position(|focus| focus == &current).unwrap_or(0);
        let next = if forward {
            ring.iter().cycle().nth(index + 1)
        } else {
            ring.iter().rev().cycle().nth(ring.len() - index)
        };
        top.focus = next.cloned().unwrap_or(current);
        Ok(Some(DialogEvent::FocusChanged))
    }
}

/// Return the frozen contract of the dialog id.
fn spec_of(id: &str) -> Result<&'static DialogSpec, DialogError> {
    spec::spec(id).ok_or(DialogError::UnknownDialog)
}

/// Return the control that holds focus when the dialog opens.
fn initial_focus(spec: &DialogSpec) -> Focus {
    let Some(field) = spec.first_field else {
        return Focus::Button(primary_button(spec));
    };
    Focus::Field(field)
}

/// Return the button Enter activates; a destructive dialog activates its cancel button.
fn primary_button(spec: &DialogSpec) -> Button {
    if spec.destructive {
        spec.cancel
    } else {
        spec.default
    }
}

/// Return the focus ring of the dialog: its first field, then its buttons.
fn focusables(spec: &DialogSpec) -> Vec<Focus> {
    let mut ring = Vec::with_capacity(spec.buttons.len() + 1);
    if let Some(field) = spec.first_field {
        ring.push(Focus::Field(field));
    }
    ring.extend(spec.buttons.iter().copied().map(Focus::Button));
    ring
}

/// Return true when the dialog's frozen window declares the field as a settable control.
fn knows_field(dialog: &str, field: &str) -> bool {
    ids::fields()
        .iter()
        .any(|entry| entry.window == dialog && entry.id == field)
}

#[cfg(test)]
mod tests;
