// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use craft::egui::{Context, Event, Id, Key as EguiKey, Modifiers, RawInput};
use craft::host::target::Os;
use craft::menu::{Chord, CommandRef, Item, ItemKind, ItemRole, MenuBar, MenuRole, Node};
use rivercrossing_store::AppSettings;

use super::{NativeBar, action_for, bar, chord, take_keys};
use crate::ui::action::id::{
    ActionId, MI_DENSITY_COMFORTABLE, MI_DENSITY_COMPACT, MI_LAPS_BOARD, MI_NEW_RIDE,
    MI_SHOW_TIMES, MI_STANDINGS, MI_START_RIDE, MI_TEXT_SIZE_LARGE, MI_UNDO_CROSSING, WX_ID_ABOUT,
    WX_ID_EXIT, WX_ID_PREFERENCES,
};
use crate::ui::action::spec::{CommandId, Key, Shortcut};
use crate::ui::state::AppState;

// ------------------------------------------------------------- fixtures

/// Return the bar built for a state whose settings are *settings*.
fn bar_with(settings: AppSettings) -> MenuBar {
    bar(&AppState::new(settings, PathBuf::new()))
}

/// Return the item that runs *id*, cloned out of the bar.
fn item(bar: &MenuBar, id: ActionId) -> Item {
    let command = CommandRef::new(id.as_str());
    bar.find(&command)
        .cloned()
        .expect("the registry row is in the bar")
}

/// Return raw input carrying one press of *key* held with *modifiers*.
fn press(key: EguiKey, modifiers: Modifiers) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..RawInput::default()
    }
}

/// Run one egui pass over *input* and return what *run* produced.
///
/// The pass paints nothing, so its texture deltas are dropped rather than applied.
fn pass<T>(ctx: &Context, input: RawInput, mut run: impl FnMut(&Context) -> T) -> T {
    let mut value = None;
    let output = ctx.run_ui(input, |ui| value = Some(run(ui.ctx())));
    output.drop_without_applying_deltas();
    value.expect("the pass ran")
}

// ------------------------------------------------------------- the model

#[test]
fn test_bar_has_the_sections_in_the_registry_order() {
    // Arrange
    let state = AppState::blank();

    // Act
    let bar = bar(&state);

    // Assert
    let titles: Vec<&str> = bar.menus.iter().map(|menu| menu.title.as_str()).collect();
    assert_eq!(
        titles,
        ["File", "Ride", "Riders", "Cards", "Results", "View", "Help"]
    );
}

#[test]
fn test_bar_gives_the_file_view_and_help_sections_their_roles() {
    // Act
    let bar = bar(&AppState::blank());

    // Assert
    let titled = |role| bar.menu(role).map(|menu| menu.title.clone());
    assert_eq!(titled(MenuRole::File), Some("File".to_owned()));
    assert_eq!(titled(MenuRole::View), Some("View".to_owned()));
    assert_eq!(titled(MenuRole::Help), Some("Help".to_owned()));
    assert_eq!(titled(MenuRole::Other), Some("Ride".to_owned()));
}

#[test]
fn test_bar_tags_the_about_settings_and_quit_items() {
    // Act
    let bar = bar(&AppState::blank());

    // Assert
    assert_eq!(item(&bar, WX_ID_ABOUT).role, Some(ItemRole::About));
    assert_eq!(item(&bar, WX_ID_PREFERENCES).role, Some(ItemRole::Settings));
    assert_eq!(item(&bar, WX_ID_EXIT).role, Some(ItemRole::Quit));
    assert_eq!(item(&bar, MI_NEW_RIDE).role, None);
}

#[test]
fn test_bar_takes_the_row_labels_from_the_registry() {
    // Act
    let bar = bar(&AppState::blank());

    // Assert
    assert_eq!(item(&bar, MI_UNDO_CROSSING).label, "Undo Last Crossing");
    assert_eq!(item(&bar, WX_ID_ABOUT).label, "About RiverCrossing");
}

#[test]
fn test_bar_gives_the_documented_shortcuts_their_chords() {
    // Act
    let bar = bar(&AppState::blank());

    // Assert
    let undo = Shortcut {
        key: Key::Z,
        command: true,
    };
    let standings = Shortcut {
        key: Key::F5,
        command: false,
    };
    assert_eq!(item(&bar, MI_UNDO_CROSSING).shortcut, chord(undo));
    assert_eq!(item(&bar, MI_STANDINGS).shortcut, chord(standings));
    assert_eq!(item(&bar, MI_NEW_RIDE).shortcut, None);
}

#[test]
fn test_bar_marks_the_check_items_from_the_settings() {
    // Arrange
    let mut settings = rivercrossing_store::default_settings();
    settings.publish_laps_board = false;
    settings.publish_show_times = true;

    // Act
    let bar = bar_with(settings);

    // Assert
    assert_eq!(item(&bar, MI_LAPS_BOARD).kind, ItemKind::Check(false));
    assert_eq!(item(&bar, MI_SHOW_TIMES).kind, ItemKind::Check(true));
}

#[test]
fn test_bar_marks_the_saved_density_and_text_size_radios() {
    // Arrange
    let mut settings = rivercrossing_store::default_settings();
    settings.density = "comfortable".to_owned();
    settings.text_size = "large".to_owned();

    // Act
    let bar = bar_with(settings);

    // Assert
    assert_eq!(
        item(&bar, MI_DENSITY_COMFORTABLE).kind,
        ItemKind::Radio(true)
    );
    assert_eq!(item(&bar, MI_DENSITY_COMPACT).kind, ItemKind::Radio(false));
    assert_eq!(item(&bar, MI_TEXT_SIZE_LARGE).kind, ItemKind::Radio(true));
}

#[test]
fn test_bar_disables_the_rows_the_gate_refuses() {
    // Act
    let bar = bar(&AppState::blank());

    // Assert
    assert!(item(&bar, MI_NEW_RIDE).enabled, "New Ride is enabled");
    assert!(
        !item(&bar, MI_START_RIDE).enabled,
        "Start Ride needs an open ride"
    );
    assert!(
        !item(&bar, MI_UNDO_CROSSING).enabled,
        "Undo needs a running ride with a crossing"
    );
}

#[test]
fn test_bar_keeps_the_group_separator_before_the_publish_checks() {
    // Act
    let bar = bar(&AppState::blank());

    // Assert
    let results = bar
        .menus
        .iter()
        .find(|menu| menu.title == "Results")
        .expect("the Results menu is in the bar");
    let command = CommandRef::new(MI_SHOW_TIMES.as_str());
    let at = results
        .children
        .iter()
        .position(|node| matches!(node, Node::Item(row) if row.command == command))
        .expect("the first publish check is in the Results menu");
    assert!(
        matches!(
            results.children.get(at.saturating_sub(1)),
            Some(Node::Separator)
        ),
        "the publish checks open a group"
    );
}

// ------------------------------------------------------------- the chords

#[test]
fn test_chord_of_a_command_shortcut_uses_the_command_modifier() {
    // Arrange
    let shortcut = Shortcut {
        key: Key::Z,
        command: true,
    };

    // Act
    let chord = chord(shortcut);

    // Assert: the command modifier is Ctrl off macOS and ⌘ on it.
    assert_eq!(
        chord.map(|chord| chord.display(Os::Linux)),
        Some("Ctrl+Z".to_owned())
    );
    assert_eq!(
        chord.map(|chord| chord.display(Os::Mac)),
        Some("⌘Z".to_owned())
    );
}

#[test]
fn test_chord_of_a_bare_shortcut_has_no_modifier() {
    // Arrange
    let shortcut = Shortcut {
        key: Key::F1,
        command: false,
    };

    // Act
    let chord = chord(shortcut);

    // Assert
    assert_eq!(chord, Chord::parse("F1").ok());
}

// ------------------------------------------------------------- the clicks

#[test]
fn test_action_for_a_registry_id_returns_the_action() {
    // Act
    let action = action_for("mi_new_ride");

    // Assert
    assert_eq!(action, Some(MI_NEW_RIDE));
}

#[test]
fn test_action_for_a_builtin_menu_id_returns_none() {
    // Act
    let action = action_for("craft-menu.hide");

    // Assert
    assert_eq!(action, None);
}

// ------------------------------------------------------------- the keys

#[test]
fn test_take_keys_returns_the_undo_action_for_the_command_z() {
    // Arrange
    let state = AppState::blank();
    let ctx = Context::default();

    // Act
    let fired = pass(&ctx, press(EguiKey::Z, Modifiers::COMMAND), |ctx| {
        take_keys(ctx, &state)
    });

    // Assert
    assert_eq!(fired, vec![MI_UNDO_CROSSING]);
}

#[test]
fn test_take_keys_returns_the_standings_action_for_f5() {
    // Arrange
    let state = AppState::blank();
    let ctx = Context::default();

    // Act
    let fired = pass(&ctx, press(EguiKey::F5, Modifiers::NONE), |ctx| {
        take_keys(ctx, &state)
    });

    // Assert
    assert_eq!(fired, vec![MI_STANDINGS]);
}

#[test]
fn test_take_keys_returns_nothing_without_a_press() {
    // Arrange
    let state = AppState::blank();
    let ctx = Context::default();

    // Act
    let fired = pass(&ctx, RawInput::default(), |ctx| take_keys(ctx, &state));

    // Assert
    assert_eq!(fired, Vec::<ActionId>::new());
}

#[test]
fn test_take_keys_ignores_a_press_while_a_text_field_has_the_keys() {
    // Arrange
    let state = AppState::blank();
    let ctx = Context::default();
    ctx.memory_mut(|memory| memory.request_focus(Id::unique("plate_input")));

    // Act
    let fired = pass(&ctx, press(EguiKey::F5, Modifiers::NONE), |ctx| {
        take_keys(ctx, &state)
    });

    // Assert
    assert_eq!(fired, Vec::<ActionId>::new());
}

#[test]
fn test_take_keys_ignores_a_press_under_a_modal_dialog() {
    // Arrange
    let mut state = AppState::blank();
    let _ = state.dialogs.open("settings_dlg", None);
    let ctx = Context::default();

    // Act
    let fired = pass(&ctx, press(EguiKey::F5, Modifiers::NONE), |ctx| {
        take_keys(ctx, &state)
    });

    // Assert
    assert_eq!(fired, Vec::<ActionId>::new());
}

#[test]
fn test_take_keys_ignores_a_press_while_a_native_box_is_pending() {
    // Arrange
    let mut state = AppState::blank();
    state.pending_native = Some(crate::ui::state::PendingNative {
        dialog: crate::ui::action::spec::NativeDialog::Confirm,
        then: crate::ui::state::NativeTask::Command(CommandId::UndoCrossing),
    });
    let ctx = Context::default();

    // Act
    let fired = pass(&ctx, press(EguiKey::F5, Modifiers::NONE), |ctx| {
        take_keys(ctx, &state)
    });

    // Assert
    assert_eq!(fired, Vec::<ActionId>::new());
}

// ------------------------------------------------------------- the driver

#[test]
fn test_a_new_native_bar_is_not_installed() {
    // Arrange
    let bar = NativeBar::default();

    // Act
    let installed = bar.is_installed();

    // Assert
    assert!(!installed, "no platform menu bar is installed yet");
}

#[test]
fn test_drive_runs_the_documented_accelerator_without_a_native_bar() {
    // Arrange
    let mut state = AppState::blank();
    let ctx = Context::default();
    let mut bar = NativeBar::default();

    // Act
    pass(&ctx, press(EguiKey::F1, Modifiers::NONE), |ctx| {
        bar.drive(ctx, &mut state);
    });

    // Assert
    assert_eq!(state.pending_command, Some(CommandId::OpenUserGuide));
}

#[test]
fn test_drive_leaves_the_state_alone_without_input() {
    // Arrange
    let mut state = AppState::blank();
    let ctx = Context::default();
    let mut bar = NativeBar::default();

    // Act
    pass(&ctx, RawInput::default(), |ctx| {
        bar.drive(ctx, &mut state);
    });

    // Assert
    assert_eq!(state.pending_command, None);
    assert_eq!(state.dialogs.top(), None);
}
