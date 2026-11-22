// SPDX-License-Identifier: GPL-3.0-only

use pretty_assertions::assert_eq;

use super::HandsError;

#[test]
fn test_invalid_card_count_displays_the_count() {
    // Arrange
    let error = HandsError::InvalidCardCount { count: 4 };

    // Act
    let text = error.to_string();

    // Assert
    assert_eq!(text, "a hand must have exactly 5 cards, got 4");
}
