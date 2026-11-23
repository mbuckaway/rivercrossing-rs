// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the scoring enums.

use pretty_assertions::assert_eq;

use super::ZeroLapMode;

#[test]
fn test_zero_lap_mode_variants_are_distinct() {
    assert_ne!(ZeroLapMode::Rank, ZeroLapMode::Dns);
    assert_ne!(ZeroLapMode::Dns, ZeroLapMode::Hide);
    assert_ne!(ZeroLapMode::Rank, ZeroLapMode::Hide);
}

#[test]
fn test_zero_lap_mode_debug_names_read_as_their_members() {
    assert_eq!(format!("{:?}", ZeroLapMode::Rank), "Rank");
    assert_eq!(format!("{:?}", ZeroLapMode::Dns), "Dns");
    assert_eq!(format!("{:?}", ZeroLapMode::Hide), "Hide");
}
