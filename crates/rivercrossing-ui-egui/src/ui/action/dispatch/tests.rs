// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the pure dispatch: the target effect, the disabled gate and the unknown id.

use rstest::rstest;

use super::{DispatchError, dispatch};
use crate::ui::action::effect::Effect;
use crate::ui::action::enable::GateState;
use crate::ui::action::id::{
    ActionId, MI_BACKUP_NOW, MI_OPEN_LIBRARY, MI_SHOW_TIMES, MI_START_RIDE, MI_ZOOM_100, WX_ID_EXIT,
};
use crate::ui::action::spec::{CheckKind, CommandId, NativeDialog};

#[rstest]
#[case(MI_OPEN_LIBRARY, Effect::OpenWindow("ride_library_dlg"))]
#[case(MI_BACKUP_NOW, Effect::RunCommand(CommandId::BackupNow))]
#[case(MI_SHOW_TIMES, Effect::ToggleCheck(CheckKind::PublishShowTimes))]
#[case(MI_ZOOM_100, Effect::SetZoom(100))]
#[case(
    WX_ID_EXIT,
    Effect::OpenNative {
        dialog: NativeDialog::Confirm,
        then: CommandId::AppQuit,
    }
)]
fn test_dispatch_returns_the_target_effect(#[case] id: ActionId, #[case] expected: Effect) {
    // Arrange
    let gate = GateState::default();

    // Act
    let result = dispatch(id, &gate);

    // Assert
    assert_eq!(result, Ok(expected));
}

#[test]
fn test_dispatch_refuses_a_disabled_action() {
    // Arrange
    let gate = GateState::default();

    // Act
    let result = dispatch(MI_START_RIDE, &gate);

    // Assert
    assert_eq!(result, Err(DispatchError::Disabled(MI_START_RIDE)));
}

#[test]
fn test_dispatch_refuses_an_unknown_id() {
    // Arrange
    let unknown = ActionId::from_name("not_an_action");
    let gate = GateState::default();

    // Act
    let result = dispatch(unknown, &gate);

    // Assert
    assert_eq!(result, Err(DispatchError::UnknownAction(unknown)));
}
