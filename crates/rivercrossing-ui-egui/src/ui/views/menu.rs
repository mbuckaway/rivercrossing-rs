// SPDX-License-Identifier: GPL-3.0-only

//! The §15 menu bar, built from the action registry and rendered by `craft::ui::menu::MenuBar`.
//!
//! Every row follows the registry: its label, its shortcut text, its check or radio mark and its
//! enabled state. An action whose target this release does not build stays visible and disabled, so
//! the operator never sees a command that silently does nothing. Activation dispatches the frozen
//! id through [`AppState::act`], the same path the console buttons and the automation channel use.

use std::cell::RefCell;

use craft::egui::Ui;
use craft::ui::contract::{Props as _, Show as _};
use craft::ui::menu::{MenuBar, MenuItem, MenuTitle};

use crate::ui::action::enable::enabled;
use crate::ui::action::id::ActionId;
use crate::ui::action::spec::{ACTIONS, CheckKind, MenuSection, Shortcut, Target, section};
use crate::ui::state::AppState;

/// One menu row as the registry resolves it for this app state.
#[derive(Clone, Debug)]
pub(crate) struct MenuRow {
    /// The §15 menu that holds the row.
    pub(crate) section: MenuSection,
    /// The frozen action id.
    pub(crate) id: ActionId,
    /// The row's label.
    pub(crate) label: &'static str,
    /// The shortcut text, when the row has one.
    pub(crate) shortcut: Option<String>,
    /// Whether the row accepts activation.
    pub(crate) enabled: bool,
    /// The check mark's state, for a checkable row.
    pub(crate) checked: Option<bool>,
    /// The radio mark's state, for a zoom row.
    pub(crate) radio: Option<bool>,
    /// Whether a separator stands above the row.
    pub(crate) separator_before: bool,
}

/// Return every menu row, in §15 table order.
///
/// A row is enabled when the §15 gate allows it and this release builds its target.
pub(crate) fn rows(state: &AppState) -> Vec<MenuRow> {
    let gate = state.gate();
    ACTIONS
        .iter()
        .map(|row| {
            let (checked, radio) = match row.target {
                Target::Check(check) => (Some(check_value(state, check)), None),
                Target::Zoom(percent) => (None, Some(state.settings.zoom_percent == percent)),
                _ => (None, None),
            };
            MenuRow {
                section: section(row.id),
                id: row.id,
                label: row.label,
                shortcut: row.shortcut.map(Shortcut::text),
                enabled: state.implemented(row.id) && enabled(row.id, &gate),
                checked,
                radio,
                separator_before: starts_a_group(row.id),
            }
        })
        .collect()
}

/// Show the menu bar and dispatch each activated row.
pub(crate) fn show(ui: &mut Ui, state: &mut AppState) {
    let rows = rows(state);
    let activated: RefCell<Vec<ActionId>> = RefCell::new(Vec::new());
    {
        let activated = &activated;
        let mut titles: Vec<MenuTitle<'_>> = Vec::with_capacity(MenuSection::ALL.len());
        for section in MenuSection::ALL {
            let mine: Vec<MenuRow> = rows
                .iter()
                .filter(|row| row.section == section)
                .cloned()
                .collect();
            titles.push(MenuTitle::new(section.title(), move |ui| {
                for row in &mine {
                    if row.separator_before {
                        ui.separator();
                    }
                    let mut item = MenuItem::new(row.label.to_owned())
                        .enabled(row.enabled)
                        .keep_open(row.checked.is_some() || row.radio.is_some());
                    if let Some(checked) = row.checked {
                        item = item.check(checked);
                    }
                    if let Some(radio) = row.radio {
                        item = item.radio_mark(radio);
                    }
                    if let Some(shortcut) = &row.shortcut {
                        item = item.shortcut(shortcut.clone());
                    }
                    let ran = ui.push_id(row.id.as_str(), |ui| item.show(ui).ran).inner;
                    if ran {
                        activated.borrow_mut().push(row.id);
                    }
                }
            }));
        }
        MenuBar::new("Main menu", titles).show(ui);
    }
    for id in activated.into_inner() {
        let _ = state.act(id);
    }
}

/// Return the persisted value of a checkable row.
fn check_value(state: &AppState, check: CheckKind) -> bool {
    let settings = &state.settings;
    match check {
        CheckKind::PublishShowTimes => settings.publish_show_times,
        CheckKind::LapsBoard => settings.publish_laps_board,
        CheckKind::TimeBoard => settings.publish_time_board,
        CheckKind::FullField => settings.publish_full_field,
        CheckKind::AllCards => settings.publish_all_cards,
        CheckKind::ShowDnsRiders => settings.show_dns_riders,
        CheckKind::ShowTotalTimes => settings.show_total_times,
        CheckKind::ShowLapTime => settings.show_lap_time,
    }
}

/// Return whether a separator stands above the row.
///
/// The six publish options sit after a separator, and so does the View menu's zoom block.
fn starts_a_group(id: ActionId) -> bool {
    use crate::ui::action::id::{MI_SHOW_TIMES, MI_ZOOM_90};
    id == MI_SHOW_TIMES || id == MI_ZOOM_90
}
