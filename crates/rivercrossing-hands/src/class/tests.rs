// SPDX-License-Identifier: GPL-3.0-only

use super::HandClass;

#[test]
fn test_hand_class_orders_straight_below_flush_and_royal_above_straight_flush() {
    // Act & Assert
    assert!(HandClass::Straight < HandClass::Flush);
    assert!(HandClass::StraightFlush < HandClass::RoyalFlush);
    assert!(HandClass::RoyalFlush < HandClass::FiveOfAKind);
}

#[test]
fn test_hand_class_from_name_reads_the_golden_name() {
    // Act
    let royal = HandClass::from_name("ROYAL_FLUSH");
    let unknown = HandClass::from_name("royal_flush");

    // Assert
    assert_eq!(royal, Some(HandClass::RoyalFlush));
    assert_eq!(unknown, None);
}

#[test]
fn test_hand_class_name_spells_the_golden_name() {
    // Act
    let name = HandClass::TwoPair.name();

    // Assert
    assert_eq!(name, "TWO_PAIR");
}
