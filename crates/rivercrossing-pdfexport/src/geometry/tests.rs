// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the print geometry: every oracle constant and the column widths.

use pretty_assertions::assert_eq;

use super::{
    A4, A4_HEIGHT_PT, A4_WIDTH_PT, AVG_COL_IN, BEST_LAP_COL_IN, CARD_BOX_BORDER, CARD_BOX_FILL,
    CARD_BOX_LINE_PT, CARD_COMPACT_GUARD_IN, CARD_COMPACT_SCALE, CARD_FULL_GUARD_IN,
    CARD_FULL_SCALE, CARD_GAP_IN, CARD_PAD_IN, CARDS_COL_IN, COVER_LOGO_HEADER_RESERVE_IN,
    COVER_LOGO_WIDTH_IN, Color, DEEP_STEEL, ENTRY_COL_IN, ENTRY_COL_WIDE_IN, FIELD_BEST_COL_IN,
    FOOTER_GAP_IN, FOOTER_RULE_LINE_PT, HEADING_GUARD_IN, INK, INLINE_LOGO_GAP_IN, INLINE_LOGO_IN,
    LAPS_BOARD_LAPS_IN, LAPS_COL_IN, LAPS_COL_TIME_IN, LAPS_COL_WIDE_IN, LETTER, LETTER_HEIGHT_PT,
    LETTER_WIDTH_PT, MARGIN_IN, PLACE_COL_IN, PLACE_COL_WIDE_IN, PLATE_COL_IN, PLATE_COL_WIDE_IN,
    PODIUM_GUARD_IN, POINTS_PER_INCH, POSTER_HEADING_GUARD_IN, REGISTRATION_LINE_PT,
    REGISTRATION_TICK_IN, ROW_GUARD_IN, ROW_HEIGHT_IN, ROW_RULE_LINE_PT, RULE_LINE_PT, STEEL,
    SUIT_RED, TEAM_ENTRY_COL_IN, TEAM_ENTRY_COL_WIDE_IN, TIME_COL_IN, TOTAL_COL_IN, TYPE_COL_IN,
    content_width_in, field_widths, laps_solo_widths, laps_team_widths, page_size,
    team_field_widths, team_top_widths, time_widths, top_solo_widths,
};

/// The Letter content width in inches, between the two 0.58in margins.
const LETTER_CONTENT_IN: f32 = 7.34;

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "expected {expected}, got {actual}"
    );
}

fn assert_widths(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len(), "width count");
    assert!(
        actual
            .iter()
            .zip(expected.iter())
            .all(|(got, want)| (got - want).abs() < 1e-4),
        "actual {actual:?}, expected {expected:?}"
    );
}

#[test]
fn test_geometry_page_constants_are_the_oracle_values() {
    // Arrange / Act / Assert
    assert_close(POINTS_PER_INCH, 72.0);
    assert_close(MARGIN_IN, 0.58);
    assert_close(FOOTER_GAP_IN, 0.82);
    assert_close(ROW_HEIGHT_IN, 0.24);
    assert_close(CARDS_COL_IN, 1.60);
    assert_close(CARD_PAD_IN, 0.04);
    assert_close(CARD_GAP_IN, 0.04);
    assert_close(CARD_BOX_LINE_PT, 0.4);
    assert_close(INLINE_LOGO_IN, 0.14);
    assert_close(INLINE_LOGO_GAP_IN, 0.03);
    assert_close(COVER_LOGO_WIDTH_IN, 0.8);
    assert_close(COVER_LOGO_HEADER_RESERVE_IN, 0.42);
    assert_close(CARD_FULL_SCALE, 0.92);
    assert_close(CARD_FULL_GUARD_IN, 1.62);
    assert_close(CARD_COMPACT_SCALE, 0.68);
    assert_close(CARD_COMPACT_GUARD_IN, 1.2);
    assert_close(POSTER_HEADING_GUARD_IN, 0.5);
    assert_close(PODIUM_GUARD_IN, 1.1);
    assert_close(ROW_GUARD_IN, 0.34);
    assert_close(HEADING_GUARD_IN, 0.45);
    assert_close(RULE_LINE_PT, 0.75);
    assert_close(ROW_RULE_LINE_PT, 0.3);
    assert_close(FOOTER_RULE_LINE_PT, 0.5);
    assert_close(REGISTRATION_LINE_PT, 0.5);
    assert_close(REGISTRATION_TICK_IN, 11.0 / 72.0);
}

#[test]
fn test_geometry_colours_are_the_industry_tokens() {
    // Arrange / Act / Assert
    assert_eq!(
        INK,
        Color {
            r: 29,
            g: 31,
            b: 32
        }
    );
    assert_eq!(
        SUIT_RED,
        Color {
            r: 192,
            g: 57,
            b: 43,
        }
    );
    assert_eq!(
        STEEL,
        Color {
            r: 65,
            g: 97,
            b: 128,
        }
    );
    assert_eq!(
        DEEP_STEEL,
        Color {
            r: 29,
            g: 45,
            b: 61,
        }
    );
    assert_eq!(
        CARD_BOX_FILL,
        Color {
            r: 233,
            g: 233,
            b: 234,
        }
    );
    assert_eq!(
        CARD_BOX_BORDER,
        Color {
            r: 219,
            g: 219,
            b: 219,
        }
    );
}

#[test]
fn test_geometry_page_sizes_are_letter_and_a4() {
    // Arrange / Act / Assert
    assert_close(LETTER_WIDTH_PT, 612.0);
    assert_close(LETTER_HEIGHT_PT, 792.0);
    assert_close(A4_WIDTH_PT, 595.28);
    assert_close(A4_HEIGHT_PT, 841.89);
    assert_close(LETTER.width_pt, 612.0);
    assert_close(LETTER.height_pt, 792.0);
    assert_close(A4.width_pt, 595.28);
    assert_close(A4.height_pt, 841.89);
}

#[test]
fn test_geometry_page_size_selects_letter_or_a4() {
    // Arrange / Act
    let letter = page_size(true);
    let a4 = page_size(false);

    // Assert
    assert_close(letter.width_pt, 612.0);
    assert_close(a4.width_pt, 595.28);
}

#[test]
fn test_geometry_content_width_removes_both_margins() {
    // Arrange / Act
    let letter = content_width_in(LETTER_WIDTH_PT);
    let a4 = content_width_in(A4_WIDTH_PT);

    // Assert
    assert_close(letter, 7.34);
    assert_close(a4, 7.107_778);
}

#[test]
fn test_geometry_top_solo_widths_without_times() {
    // Arrange / Act
    let actual = top_solo_widths(LETTER_CONTENT_IN, false);

    // Assert
    assert_widths(
        &actual,
        &[
            PLACE_COL_WIDE_IN,
            PLATE_COL_WIDE_IN,
            ENTRY_COL_WIDE_IN,
            LAPS_COL_WIDE_IN,
            CARDS_COL_IN,
            2.77,
        ],
    );
}

#[test]
fn test_geometry_top_solo_widths_with_times() {
    // Arrange / Act
    let actual = top_solo_widths(LETTER_CONTENT_IN, true);

    // Assert
    assert_widths(
        &actual,
        &[
            PLACE_COL_WIDE_IN,
            PLATE_COL_WIDE_IN,
            ENTRY_COL_WIDE_IN,
            LAPS_COL_WIDE_IN,
            TOTAL_COL_IN,
            CARDS_COL_IN,
            1.87,
        ],
    );
}

#[test]
fn test_geometry_top_team_widths_without_times() {
    // Arrange / Act
    let actual = team_top_widths(LETTER_CONTENT_IN, false);

    // Assert
    assert_widths(
        &actual,
        &[
            PLACE_COL_WIDE_IN,
            TEAM_ENTRY_COL_WIDE_IN,
            LAPS_COL_WIDE_IN,
            CARDS_COL_IN,
            3.09,
        ],
    );
}

#[test]
fn test_geometry_top_team_widths_with_times() {
    // Arrange / Act
    let actual = team_top_widths(LETTER_CONTENT_IN, true);

    // Assert
    assert_widths(
        &actual,
        &[
            PLACE_COL_WIDE_IN,
            TEAM_ENTRY_COL_WIDE_IN,
            LAPS_COL_WIDE_IN,
            TOTAL_COL_IN,
            CARDS_COL_IN,
            2.19,
        ],
    );
}

#[test]
fn test_geometry_solo_field_widths_without_times() {
    // Arrange / Act
    let actual = field_widths(LETTER_CONTENT_IN, false);

    // Assert
    assert_widths(
        &actual,
        &[
            PLACE_COL_IN,
            PLATE_COL_IN,
            ENTRY_COL_IN,
            TYPE_COL_IN,
            LAPS_COL_IN,
            4.09,
        ],
    );
}

#[test]
fn test_geometry_solo_field_widths_with_times() {
    // Arrange / Act
    let actual = field_widths(LETTER_CONTENT_IN, true);

    // Assert
    assert_widths(
        &actual,
        &[
            PLACE_COL_IN,
            PLATE_COL_IN,
            ENTRY_COL_IN,
            TYPE_COL_IN,
            LAPS_COL_IN,
            BEST_LAP_COL_IN,
            FIELD_BEST_COL_IN,
            2.44,
        ],
    );
}

#[test]
fn test_geometry_team_field_widths_without_times() {
    // Arrange / Act
    let actual = team_field_widths(LETTER_CONTENT_IN, false);

    // Assert
    assert_widths(
        &actual,
        &[
            PLACE_COL_IN,
            TEAM_ENTRY_COL_IN,
            LAPS_COL_IN,
            CARDS_COL_IN,
            3.59,
        ],
    );
}

#[test]
fn test_geometry_team_field_widths_with_times() {
    // Arrange / Act
    let actual = team_field_widths(LETTER_CONTENT_IN, true);

    // Assert
    assert_widths(
        &actual,
        &[
            PLACE_COL_IN,
            TEAM_ENTRY_COL_IN,
            LAPS_COL_IN,
            BEST_LAP_COL_IN,
            FIELD_BEST_COL_IN,
            CARDS_COL_IN,
            1.94,
        ],
    );
}

#[test]
fn test_geometry_solo_laps_widths_without_times() {
    // Arrange / Act
    let actual = laps_solo_widths(LETTER_CONTENT_IN, false);

    // Assert
    assert_widths(
        &actual,
        &[PLACE_COL_IN, PLATE_COL_IN, 5.81, LAPS_BOARD_LAPS_IN],
    );
}

#[test]
fn test_geometry_solo_laps_widths_with_times() {
    // Arrange / Act
    let actual = laps_solo_widths(LETTER_CONTENT_IN, true);

    // Assert
    assert_widths(
        &actual,
        &[
            PLACE_COL_IN,
            PLATE_COL_IN,
            4.91,
            LAPS_BOARD_LAPS_IN,
            TOTAL_COL_IN,
        ],
    );
}

#[test]
fn test_geometry_team_laps_widths_without_times() {
    // Arrange / Act
    let actual = laps_team_widths(LETTER_CONTENT_IN, false);

    // Assert
    assert_widths(&actual, &[PLACE_COL_IN, 6.39, LAPS_BOARD_LAPS_IN]);
}

#[test]
fn test_geometry_team_laps_widths_with_times() {
    // Arrange / Act
    let actual = laps_team_widths(LETTER_CONTENT_IN, true);

    // Assert
    assert_widths(
        &actual,
        &[PLACE_COL_IN, 5.49, LAPS_BOARD_LAPS_IN, TOTAL_COL_IN],
    );
}

#[test]
fn test_geometry_time_widths_split_the_fastest_board() {
    // Arrange / Act
    let actual = time_widths(LETTER_CONTENT_IN);

    // Assert
    assert_widths(
        &actual,
        &[
            PLACE_COL_IN,
            PLATE_COL_IN,
            3.51,
            LAPS_COL_TIME_IN,
            TIME_COL_IN,
            AVG_COL_IN,
        ],
    );
}
