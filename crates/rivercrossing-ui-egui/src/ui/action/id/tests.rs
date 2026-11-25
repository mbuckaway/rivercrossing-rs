// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the frozen `ActionId` values (`design/docs-md/spec.md` §15b).

use super::{ActionId, MI_OPEN_LIBRARY, MI_START_RIDE, WX_ID_ABOUT};

#[test]
fn test_action_id_as_str_returns_the_frozen_name() {
    // Act
    let library = MI_OPEN_LIBRARY.as_str();
    let about = WX_ID_ABOUT.as_str();

    // Assert
    assert_eq!(library, "mi_open_library");
    assert_eq!(about, "wxID_ABOUT");
}

#[test]
fn test_action_id_from_name_round_trips_a_frozen_id() {
    // Act
    let rebuilt = ActionId::from_name("mi_start_ride");

    // Assert
    assert_eq!(rebuilt, MI_START_RIDE);
}

#[test]
fn test_action_id_from_name_keeps_an_unknown_name() {
    // Act
    let unknown = ActionId::from_name("not_an_action");

    // Assert
    assert_eq!(unknown.as_str(), "not_an_action");
}
