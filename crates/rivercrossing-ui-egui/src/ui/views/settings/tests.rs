// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the Settings dialog's section headings and its average-speed column.

use craft::ui::sections::HeaderStrength;

use super::{AVG_SPEED_WIDTH, HEADING_STRENGTH};

#[test]
fn test_settings_headings_given_a_group_use_the_caption_strength() {
    // Arrange: the design draws a quiet heading with no rule over each group.

    // Act
    let strength = HEADING_STRENGTH;

    // Assert
    assert_eq!(strength, HeaderStrength::Caption);
}

#[test]
fn test_settings_avg_speed_given_its_field_takes_the_designs_narrow_width() {
    // Arrange: the mockup caps the average-speed field at 220 points.

    // Act
    let width = AVG_SPEED_WIDTH;

    // Assert
    assert_eq!(width.to_bits(), 220.0_f32.to_bits());
}
