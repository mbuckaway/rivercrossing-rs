// SPDX-License-Identifier: GPL-3.0-only

//! The print geometry: every layout constant and the per-board column widths.
//!
//! Values are the Python oracle's `pdfexport.py` inch and point measurements.
//! Column widths are in inches and the trailing "rest" column absorbs the
//! leftover content width, so a board always spans the printable area.

/// The points in one inch.
pub const POINTS_PER_INCH: f32 = 72.0;

/// The page margin on all four sides, in inches.
pub const MARGIN_IN: f32 = 0.58;

/// The bottom gap the automatic page break leaves for the footer, in inches.
pub const FOOTER_GAP_IN: f32 = 0.82;

/// One table row's height, in inches.
pub const ROW_HEIGHT_IN: f32 = 0.24;

/// The Best-5 card column's width, in inches.
pub const CARDS_COL_IN: f32 = 1.60;

/// One card face's horizontal padding, in inches.
pub const CARD_PAD_IN: f32 = 0.04;

/// The gap between two card faces, in inches.
pub const CARD_GAP_IN: f32 = 0.04;

/// The card face's hairline border width, in points.
pub const CARD_BOX_LINE_PT: f32 = 0.4;

/// The inline team logo's edge, in inches.
pub const INLINE_LOGO_IN: f32 = 0.14;

/// The trailing gap after an inline team logo, in inches.
pub const INLINE_LOGO_GAP_IN: f32 = 0.03;

/// The cover logo's width, in inches.
pub const COVER_LOGO_WIDTH_IN: f32 = 0.8;

/// The header height the cover reserves when a logo is drawn, in inches.
pub const COVER_LOGO_HEADER_RESERVE_IN: f32 = 0.42;

/// The poster card scale on a full-size card.
pub const CARD_FULL_SCALE: f32 = 0.92;

/// The poster page-break guard for a full-size card, in inches.
pub const CARD_FULL_GUARD_IN: f32 = 1.62;

/// The poster card scale on a compact card.
pub const CARD_COMPACT_SCALE: f32 = 0.68;

/// The poster page-break guard for a compact card, in inches.
pub const CARD_COMPACT_GUARD_IN: f32 = 1.2;

/// The poster section head's page-break guard, in inches.
pub const POSTER_HEADING_GUARD_IN: f32 = 0.5;

/// The report podium card's page-break guard, in inches.
pub const PODIUM_GUARD_IN: f32 = 1.1;

/// One table row's page-break guard, in inches.
pub const ROW_GUARD_IN: f32 = ROW_HEIGHT_IN + 0.10;

/// A section heading's page-break guard, in inches.
pub const HEADING_GUARD_IN: f32 = 0.45;

/// A section rule's line width, in points.
pub const RULE_LINE_PT: f32 = 0.75;

/// A row separator's line width, in points.
pub const ROW_RULE_LINE_PT: f32 = 0.3;

/// A footer rule's line width, in points.
pub const FOOTER_RULE_LINE_PT: f32 = 0.5;

/// One registration tick's length, in inches.
pub const REGISTRATION_TICK_IN: f32 = 11.0 / POINTS_PER_INCH;

/// A registration tick's line width, in points.
pub const REGISTRATION_LINE_PT: f32 = 0.5;

/// The Letter page width, in points.
pub const LETTER_WIDTH_PT: f32 = 612.0;

/// The Letter page height, in points.
pub const LETTER_HEIGHT_PT: f32 = 792.0;

/// The A4 page width, in points.
pub const A4_WIDTH_PT: f32 = 595.28;

/// The A4 page height, in points.
pub const A4_HEIGHT_PT: f32 = 841.89;

/// The solo top-list place column, in inches.
pub const PLACE_COL_WIDE_IN: f32 = 0.40;

/// The solo top-list plate column, in inches.
pub const PLATE_COL_WIDE_IN: f32 = 0.62;

/// The solo top-list entry column, in inches.
pub const ENTRY_COL_WIDE_IN: f32 = 1.50;

/// The solo top-list laps column, in inches.
pub const LAPS_COL_WIDE_IN: f32 = 0.45;

/// The top-list total-time column, in inches.
pub const TOTAL_COL_IN: f32 = 0.90;

/// The team top-list entry column, in inches.
pub const TEAM_ENTRY_COL_WIDE_IN: f32 = 1.80;

/// The narrow place column shared by the field and the boards, in inches.
pub const PLACE_COL_IN: f32 = 0.35;

/// The narrow plate column, in inches.
pub const PLATE_COL_IN: f32 = 0.58;

/// The solo field's entry column, in inches.
pub const ENTRY_COL_IN: f32 = 1.30;

/// The solo field's type column, in inches.
pub const TYPE_COL_IN: f32 = 0.62;

/// The narrow laps column, in inches.
pub const LAPS_COL_IN: f32 = 0.40;

/// The solo field's best-lap column, in inches.
pub const BEST_LAP_COL_IN: f32 = 0.85;

/// The team field's best-lap column, in inches.
pub const FIELD_BEST_COL_IN: f32 = 0.80;

/// The team field's entry column, in inches.
pub const TEAM_ENTRY_COL_IN: f32 = 1.40;

/// The laps board's laps column, in inches.
pub const LAPS_BOARD_LAPS_IN: f32 = 0.60;

/// The fastest board's laps column, in inches.
pub const LAPS_COL_TIME_IN: f32 = 0.90;

/// The fastest board's total column, in inches.
pub const TIME_COL_IN: f32 = 0.95;

/// The fastest board's average-lap column, in inches.
pub const AVG_COL_IN: f32 = 1.05;

/// One RGB colour, the shape every [`crate::layout::Op`] carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    /// The red channel.
    pub r: u8,
    /// The green channel.
    pub g: u8,
    /// The blue channel.
    pub b: u8,
}

impl Color {
    /// Return the colour with channels *r*, *g* and *b*.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// The ink token `#1D1F20`: body text and spades/clubs.
pub const INK: Color = Color::rgb(0x1D, 0x1F, 0x20);

/// The suit red token `#C0392B`: hearts and diamonds.
pub const SUIT_RED: Color = Color::rgb(0xC0, 0x39, 0x2B);

/// The steel token `#416180`: jokers, hand names and report furniture.
pub const STEEL: Color = Color::rgb(0x41, 0x61, 0x80);

/// The deep steel token `#1D2D3D`: the first podium place.
pub const DEEP_STEEL: Color = Color::rgb(0x1D, 0x2D, 0x3D);

/// The card face fill `#E9E9EA`.
pub const CARD_BOX_FILL: Color = Color::rgb(0xE9, 0xE9, 0xEA);

/// The card face border `#DBDBDB`.
pub const CARD_BOX_BORDER: Color = Color::rgb(0xDB, 0xDB, 0xDB);

/// One page's paper size, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
    /// The page width, in points.
    pub width_pt: f32,
    /// The page height, in points.
    pub height_pt: f32,
}

/// The Letter paper size.
pub const LETTER: PageSize = PageSize {
    width_pt: LETTER_WIDTH_PT,
    height_pt: LETTER_HEIGHT_PT,
};

/// The A4 paper size.
pub const A4: PageSize = PageSize {
    width_pt: A4_WIDTH_PT,
    height_pt: A4_HEIGHT_PT,
};

/// Return the paper size for *letter* (Letter) or A4.
#[must_use]
pub fn page_size(letter: bool) -> PageSize {
    if letter { LETTER } else { A4 }
}

/// Return the printable width between the margins, in inches.
#[must_use]
pub fn content_width_in(page_width_pt: f32) -> f32 {
    page_width_pt / POINTS_PER_INCH - 2.0 * MARGIN_IN
}

/// Append the card column and the trailing rest column to *leading*.
fn with_cards_col(leading: &[f32], content_in: f32) -> Vec<f32> {
    let used: f32 = leading.iter().sum();
    let mut widths = leading.to_vec();
    widths.push(CARDS_COL_IN);
    widths.push(content_in - used - CARDS_COL_IN);
    widths
}

/// Return the solo top-list widths: Place|Plate|Entry|Laps|[Total]|Cards|Hand.
#[must_use]
pub fn top_solo_widths(content_in: f32, show_times: bool) -> Vec<f32> {
    let mut leading = vec![
        PLACE_COL_WIDE_IN,
        PLATE_COL_WIDE_IN,
        ENTRY_COL_WIDE_IN,
        LAPS_COL_WIDE_IN,
    ];
    if show_times {
        leading.push(TOTAL_COL_IN);
    }
    with_cards_col(&leading, content_in)
}

/// Return the team top-list widths: Place|Entry|Laps|[Total]|Cards|Hand.
#[must_use]
pub fn team_top_widths(content_in: f32, show_times: bool) -> Vec<f32> {
    let mut leading = vec![PLACE_COL_WIDE_IN, TEAM_ENTRY_COL_WIDE_IN, LAPS_COL_WIDE_IN];
    if show_times {
        leading.push(TOTAL_COL_IN);
    }
    with_cards_col(&leading, content_in)
}

/// Return the solo field widths: Place|Plate|Entry|Type|Laps|[times]|Hand.
#[must_use]
pub fn field_widths(content_in: f32, show_times: bool) -> Vec<f32> {
    let mut widths = vec![
        PLACE_COL_IN,
        PLATE_COL_IN,
        ENTRY_COL_IN,
        TYPE_COL_IN,
        LAPS_COL_IN,
    ];
    if show_times {
        widths.push(BEST_LAP_COL_IN);
        widths.push(FIELD_BEST_COL_IN);
    }
    let used: f32 = widths.iter().sum();
    widths.push(content_in - used);
    widths
}

/// Return the team field widths: Place|Entry|Laps|[times]|Cards|Hand.
#[must_use]
pub fn team_field_widths(content_in: f32, show_times: bool) -> Vec<f32> {
    let mut leading = vec![PLACE_COL_IN, TEAM_ENTRY_COL_IN, LAPS_COL_IN];
    if show_times {
        leading.push(BEST_LAP_COL_IN);
        leading.push(FIELD_BEST_COL_IN);
    }
    with_cards_col(&leading, content_in)
}

/// Return the solo laps-board widths: #|Plate|Entry|Laps|[Total].
#[must_use]
pub fn laps_solo_widths(content_in: f32, show_times: bool) -> Vec<f32> {
    let mut widths = vec![PLACE_COL_IN, PLATE_COL_IN, LAPS_BOARD_LAPS_IN];
    if show_times {
        widths.push(TOTAL_COL_IN);
    }
    let used: f32 = widths.iter().sum();
    widths.insert(2, content_in - used);
    widths
}

/// Return the team laps-board widths: #|Entry|Laps|[Total].
#[must_use]
pub fn laps_team_widths(content_in: f32, show_times: bool) -> Vec<f32> {
    let mut widths = vec![PLACE_COL_IN, LAPS_BOARD_LAPS_IN];
    if show_times {
        widths.push(TOTAL_COL_IN);
    }
    let used: f32 = widths.iter().sum();
    widths.insert(1, content_in - used);
    widths
}

/// Return the fastest board widths: #|Plate|Entry|Laps|Total|Avg lap.
#[must_use]
pub fn time_widths(content_in: f32) -> Vec<f32> {
    let mut widths = vec![
        PLACE_COL_IN,
        PLATE_COL_IN,
        LAPS_COL_TIME_IN,
        TIME_COL_IN,
        AVG_COL_IN,
    ];
    let used: f32 = widths.iter().sum();
    widths.insert(2, content_in - used);
    widths
}

#[cfg(test)]
mod tests;
