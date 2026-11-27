// SPDX-License-Identifier: GPL-3.0-only

//! The native macOS menu bar and the app's documented keyboard accelerators.
//!
//! The bar is built from the same [`views::menu::rows`] the in-window bar renders, so the native
//! menus cannot drift from the frozen §15 registry: a row's label, shortcut, check or radio mark
//! and its enabled state come from one function. [`craft::menu::mac_layout`] moves About, Settings
//! and Quit into the macOS app menu and assigns ⌘Q, and [`craft::menu::mac::install`] hands the
//! result to `AppKit` once, inside eframe's creator closure.
//!
//! [`NativeBar::drive`] runs the driver once per `logic` pass: the accelerators, then `poll`,
//! `set_gate`, `take_clicks` and a sync. A clicked command is a frozen action id, so it runs
//! through [`AppState::act`] like a click in the in-window bar. Off macOS `install` returns `None`
//! and the app keeps its in-window bar.

use std::io::Write;

use craft::egui::{Context, Key as EguiKey, Modifiers};
use craft::menu::mac::{MacOptions, install_app_menu, install_with};
use craft::menu::{
    Chord, CommandRef, Gate, Item, ItemRole, Layout, LayoutOptions, Menu, MenuBar, MenuRole,
    NativeMenu, Node, mac_layout,
};

use super::action::id::{ActionId, WX_ID_ABOUT, WX_ID_EXIT, WX_ID_PREFERENCES};
use super::action::spec::{ACTIONS, Key, MenuSection, Shortcut};
use super::state::AppState;
use super::views;
use super::window::APP_NAME;

/// The root window's native menu bar, when the platform has one installed.
#[derive(Default)]
pub(crate) struct NativeBar {
    /// The installed driver. `None` off macOS, when `AppKit` refused the menus, and in a test or a
    /// headless host, which then keeps the in-window bar.
    handle: Option<NativeMenu>,
}

impl NativeBar {
    /// Return whether a platform menu bar is installed; when it is not, the app shows the
    /// in-window bar instead.
    pub(crate) fn is_installed(&self) -> bool {
        self.handle.is_some()
    }

    /// Install the native menu bar for this app. Call it once, inside eframe's creator closure, so
    /// winit's default menu does not stay up.
    pub(crate) fn install(&mut self, ctx: &Context, state: &AppState) {
        // One window: AppKit would list it again in the Window menu.
        let options = MacOptions::default().window_list(false);
        self.handle = install_with(ctx, &layout(state).bar, options);
        if self.handle.is_some() {
            #[cfg(target_os = "macos")]
            {
                let _ = super::macos::retitle_app_menu(APP_NAME);
            }
            return;
        }
        // winit's default Quit ends the process without asking; the minimal app menu asks first.
        if let Err(error) = install_app_menu(ctx, APP_NAME) {
            writeln!(
                std::io::stderr(),
                "warning: the app menu did not install: {error}"
            )
            .ok();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = super::macos::retitle_app_menu(APP_NAME);
        }
    }

    /// Run one pass of the driver: the documented accelerators, then the native menus.
    pub(crate) fn drive(&mut self, ctx: &Context, state: &mut AppState) {
        for id in take_keys(ctx, state) {
            let _ = state.act(id);
        }
        let Some(native) = self.handle.as_mut() else {
            return;
        };
        // `poll` is documented for eframe's `raw_input_hook`. Called inside the pass (`logic`), the
        // window host repeats the pass with the polled key equivalents in its input.
        native.poll(ctx);
        native.set_gate(
            Gate::default()
                .modal(state.dialogs.top().is_some() || state.pending_native.is_some())
                .text_editing(ctx.egui_wants_keyboard_input())
                .recording_shortcut(false),
        );
        for command in native.take_clicks(ctx) {
            if let Some(id) = action_for(&command.id) {
                let _ = state.act(id);
            }
        }
        // `sync` compares the fresh layout with the last one, so only a change reaches AppKit.
        native.sync(layout(state));
        // AppKit names the application menu after the process and can overwrite the installed
        // title; the call is a no-op once the title matches.
        #[cfg(target_os = "macos")]
        {
            let _ = super::macos::retitle_app_menu(APP_NAME);
        }
    }
}

/// Return the §15 menu tree for *state*: one menu per section, its rows in table order, with the
/// About, Settings and Quit items and the File, View and Help menus tagged for the macOS layout.
pub(crate) fn bar(state: &AppState) -> MenuBar {
    let rows = views::menu::rows(state);
    let mut menus = Vec::with_capacity(MenuSection::ALL.len());
    for section in MenuSection::ALL {
        let mut children = Vec::new();
        for row in rows.iter().filter(|row| row.section == section) {
            if row.separator_before {
                children.push(Node::Separator);
            }
            children.push(Node::Item(item(row)));
        }
        menus.push(Menu::new(section.title(), children).role(menu_role(section)));
    }
    let mut bar = MenuBar::new(menus);
    bar.tag_item(&CommandRef::new(WX_ID_ABOUT.as_str()), ItemRole::About);
    bar.tag_item(
        &CommandRef::new(WX_ID_PREFERENCES.as_str()),
        ItemRole::Settings,
    );
    bar.tag_item(&CommandRef::new(WX_ID_EXIT.as_str()), ItemRole::Quit);
    bar
}

/// Return the chord a row's documented shortcut names, if it parses.
///
/// The command modifier is `Cmd`, which [`Chord`] shows as ⌘ on macOS and Ctrl elsewhere. The
/// registry's shortcuts are fixed text, so a row's shortcut always parses; a parse failure would
/// leave the row without one.
pub(crate) fn chord(shortcut: Shortcut) -> Option<Chord> {
    let key = shortcut.key.as_str();
    if shortcut.command {
        Chord::parse(&format!("Cmd+{key}")).ok()
    } else {
        Chord::parse(key).ok()
    }
}

/// Return the frozen action a command id names, or `None` for a platform built-in command.
pub(crate) fn action_for(id: &str) -> Option<ActionId> {
    ACTIONS
        .iter()
        .find(|action| action.id.as_str() == id)
        .map(|action| action.id)
}

/// Consume the documented accelerators this pass pressed and return the actions to run.
///
/// A shortcut that a menu row owns runs that row's id, so the key and the menu item cannot
/// diverge. While a text field has the keys, or a modal dialog or a pending native box owns them,
/// the accelerator stays hands-off.
pub(crate) fn take_keys(ctx: &Context, state: &AppState) -> Vec<ActionId> {
    if state.dialogs.top().is_some()
        || state.pending_native.is_some()
        || ctx.egui_wants_keyboard_input()
    {
        return Vec::new();
    }
    let mut fired = Vec::new();
    ctx.input_mut(|input| {
        for row in ACTIONS {
            let Some(shortcut) = row.shortcut else {
                continue;
            };
            if input.consume_key(modifiers(shortcut), egui_key(shortcut)) {
                fired.push(row.id);
            }
        }
    });
    fired
}

/// Return the §15 menus laid out the macOS way for *state*.
fn layout(state: &AppState) -> Layout {
    mac_layout(&bar(state), &LayoutOptions::new(APP_NAME))
}

/// Return the item a resolved row becomes.
fn item(row: &views::menu::MenuRow) -> Item {
    let mut item = Item::new(row.id.as_str(), row.label).enabled(row.enabled);
    if let Some(shortcut) = row.shortcut.and_then(chord) {
        item = item.shortcut(shortcut);
    }
    if let Some(checked) = row.checked {
        item = item.check(checked);
    }
    if let Some(radio) = row.radio {
        item = item.radio(radio);
    }
    item
}

/// Return the role the macOS layout gives a section.
const fn menu_role(section: MenuSection) -> MenuRole {
    match section {
        MenuSection::File => MenuRole::File,
        MenuSection::View => MenuRole::View,
        MenuSection::Help => MenuRole::Help,
        MenuSection::Ride | MenuSection::Riders | MenuSection::Cards | MenuSection::Results => {
            MenuRole::Other
        }
    }
}

/// Return the egui modifiers the row's shortcut is pressed with.
const fn modifiers(shortcut: Shortcut) -> Modifiers {
    if shortcut.command {
        Modifiers::COMMAND
    } else {
        Modifiers::NONE
    }
}

/// Return the egui key the row's shortcut presses.
const fn egui_key(shortcut: Shortcut) -> EguiKey {
    match shortcut.key {
        Key::Z => EguiKey::Z,
        Key::F1 => EguiKey::F1,
        Key::F5 => EguiKey::F5,
    }
}

#[cfg(test)]
mod tests;
