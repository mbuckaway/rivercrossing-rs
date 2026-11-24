// SPDX-License-Identifier: GPL-3.0-only

//! The pure page model: the report and poster laid out as drawable operations.
//!
//! Coordinates are points with the origin at the top-left corner and `y`
//! growing downward (the Python oracle's fpdf2 convention). A [`Op::Text`]
//! carries the top of its cell and its left edge; the Phase 7 writer converts
//! both to the baseline and the bottom-left origin. The model draws no bytes:
//! it only measures text through [`crate::fonts`] and emits operations.

#![expect(
    clippy::too_many_arguments,
    reason = "the drawer methods carry a column slice, a row and one layout flag; bundling every pair would obscure the oracle's shape"
)]

use rivercrossing_htmlexport::sections::poster_sections;
use rivercrossing_htmlexport::types::{
    CardPair, ExportOptions, LapsBoardRow, RacePayload, ResultRow, Sections, TimeBoardRow,
};

use crate::error::PdfError;
use crate::fonts::{FontFace, text_width};
use crate::geometry::{
    CARD_BOX_BORDER, CARD_BOX_FILL, CARD_BOX_LINE_PT, CARD_COMPACT_GUARD_IN, CARD_COMPACT_SCALE,
    CARD_FULL_GUARD_IN, CARD_FULL_SCALE, CARD_GAP_IN, CARD_PAD_IN, COVER_LOGO_HEADER_RESERVE_IN,
    COVER_LOGO_WIDTH_IN, Color, DEEP_STEEL, FOOTER_GAP_IN, FOOTER_RULE_LINE_PT, HEADING_GUARD_IN,
    INK, MARGIN_IN, PODIUM_GUARD_IN, POINTS_PER_INCH, POSTER_HEADING_GUARD_IN, PageSize,
    REGISTRATION_LINE_PT, REGISTRATION_TICK_IN, ROW_GUARD_IN, ROW_HEIGHT_IN, ROW_RULE_LINE_PT,
    RULE_LINE_PT, STEEL, SUIT_RED, content_width_in, field_widths, laps_solo_widths,
    laps_team_widths, page_size, team_field_widths, team_top_widths, time_widths, top_solo_widths,
};

/// The separator between the ride name and "Official results" in the running title.
const RUNNING_TITLE_SEP: &str = " — Official results";

/// The PNG file signature every organizer logo must start with.
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The card face's word when the pair is a joker.
const JOKER_TEXT: &str = "★ JOKER";

/// One drawable operation, in points with a top-left origin.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// A run of text drawn in one face.
    Text {
        /// The left edge, in points.
        x: f32,
        /// The top of the text's cell, in points.
        y: f32,
        /// The face that draws the text.
        font: FontFace,
        /// The font size, in points.
        size: f32,
        /// The text colour.
        color: Color,
        /// The text itself.
        text: String,
    },
    /// A filled rectangle, such as a card face.
    FilledRect {
        /// The left edge, in points.
        x: f32,
        /// The top edge, in points.
        y: f32,
        /// The width, in points.
        w: f32,
        /// The height, in points.
        h: f32,
        /// The fill colour.
        fill: Color,
    },
    /// A straight line, such as a rule or a registration tick.
    Line {
        /// The start x, in points.
        x1: f32,
        /// The start y, in points.
        y1: f32,
        /// The end x, in points.
        x2: f32,
        /// The end y, in points.
        y2: f32,
        /// The line colour.
        color: Color,
        /// The line width, in points.
        width_pt: f32,
    },
    /// A PNG image, such as the organizer logo.
    Image {
        /// The left edge, in points.
        x: f32,
        /// The top edge, in points.
        y: f32,
        /// The drawn width, in points.
        w: f32,
        /// The drawn height, in points; `0.0` keeps the PNG's own aspect.
        h: f32,
        /// The PNG bytes.
        png: Vec<u8>,
    },
}

/// One page: its paper size and the operations drawn on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// The page width, in points.
    pub width_pt: f32,
    /// The page height, in points.
    pub height_pt: f32,
    /// The operations, in drawing order.
    pub ops: Vec<Op>,
}

impl Page {
    /// Return an empty page of *`width_pt`* by *`height_pt`*.
    fn empty(width_pt: f32, height_pt: f32) -> Self {
        Self {
            width_pt,
            height_pt,
            ops: Vec::new(),
        }
    }
}

/// A cell or box's position, in inches.
#[derive(Debug, Clone, Copy)]
struct Pos {
    x: f32,
    y: f32,
}

/// A rectangle, in inches.
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// A horizontal or vertical run, in inches.
#[derive(Debug, Clone, Copy)]
struct Seg {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

/// A column span, in inches.
#[derive(Debug, Clone, Copy)]
struct Span {
    x: f32,
    w: f32,
}

/// A stroke's colour and width.
#[derive(Debug, Clone, Copy)]
struct Stroke {
    color: Color,
    width_pt: f32,
}

/// One text run's face, size, colour and alignment.
#[derive(Debug, Clone, Copy)]
enum Align {
    Left,
    Right,
    Center,
}

/// One text run's face, size, colour and alignment.
#[derive(Debug, Clone, Copy)]
struct TextStyle {
    font: FontFace,
    size: f32,
    color: Color,
    align: Align,
}

/// A paragraph's style and leading.
#[derive(Debug, Clone, Copy)]
struct Lead {
    style: TextStyle,
    leading_in: f32,
}

/// One boxed card face's sizing.
#[derive(Debug, Clone, Copy)]
struct CardBox {
    size_pt: f32,
    height_in: f32,
}

/// The whole-hand sub-row's sizing.
#[derive(Debug, Clone, Copy)]
struct DrawnRow {
    size_pt: f32,
    leading_in: f32,
    gap_in: f32,
}

/// A poster card's scale and page-break guard.
#[derive(Debug, Clone, Copy)]
struct CardGeom {
    scale: f32,
    guard: f32,
}

/// One board row's numbering and plate form.
#[derive(Debug, Clone, Copy)]
struct Placement {
    place: usize,
    show_plate: bool,
}

/// The footer furniture every page carries.
#[derive(Debug, Clone)]
struct Footer {
    credits: String,
    generated: String,
    numbered: bool,
}

/// One page's number and the document total.
#[derive(Debug, Clone, Copy)]
struct Paging {
    number: usize,
    total: usize,
}

/// One table cell's column and row position.
#[derive(Debug, Clone, Copy)]
struct Slot {
    index: usize,
    y: f32,
}

const ROW_STYLE: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 7.5,
    color: INK,
    align: Align::Left,
};
const ROW_BOLD: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 7.5,
    color: INK,
    align: Align::Left,
};
const ROW_RIGHT: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 7.5,
    color: INK,
    align: Align::Right,
};
const FIELD_STYLE: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 7.0,
    color: INK,
    align: Align::Left,
};
const FIELD_BOLD: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 7.0,
    color: INK,
    align: Align::Left,
};
const HAND_STYLE: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 6.5,
    color: STEEL,
    align: Align::Left,
};
const PODIUM_HAND_STYLE: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 9.5,
    color: STEEL,
    align: Align::Left,
};
const BOARD_TOTAL: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 7.5,
    color: INK,
    align: Align::Left,
};
const TABLE_HEAD: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 7.0,
    color: STEEL,
    align: Align::Left,
};
const SECTION_HEAD: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 13.0,
    color: INK,
    align: Align::Left,
};
const SUBSECTION_HEAD: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 11.0,
    color: INK,
    align: Align::Left,
};
const CAPTION: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 7.5,
    color: INK,
    align: Align::Left,
};
const COVER_KICKER: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 10.0,
    color: STEEL,
    align: Align::Left,
};
const COVER_TITLE: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 26.0,
    color: INK,
    align: Align::Left,
};
const COVER_META: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 9.5,
    color: INK,
    align: Align::Left,
};
const COVER_COUNTERS: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 9.5,
    color: INK,
    align: Align::Right,
};
const COVER_LABELS: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 7.5,
    color: INK,
    align: Align::Right,
};
const DISCLAIMER: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 8.5,
    color: INK,
    align: Align::Left,
};
const SELF_TEST: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 8.5,
    color: STEEL,
    align: Align::Left,
};
const FULL_FIELD_NOTE: TextStyle = TextStyle {
    font: FontFace::Glyph,
    size: 7.0,
    color: INK,
    align: Align::Left,
};
const PODIUM_NAME: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 13.0,
    color: INK,
    align: Align::Left,
};
const PODIUM_SUBTITLE: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 8.0,
    color: INK,
    align: Align::Left,
};
const POSTER_META: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 9.0,
    color: STEEL,
    align: Align::Left,
};
const POSTER_TITLE: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 22.0,
    color: INK,
    align: Align::Left,
};
const POSTER_SECTION: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 13.0,
    color: INK,
    align: Align::Left,
};
const RUNNING_TITLE: TextStyle = TextStyle {
    font: FontFace::Heading,
    size: 11.0,
    color: INK,
    align: Align::Left,
};
const FOOTER_TEXT: TextStyle = TextStyle {
    font: FontFace::Body,
    size: 7.0,
    color: STEEL,
    align: Align::Left,
};
const REPORT_DRAWN: DrawnRow = DrawnRow {
    size_pt: 6.5,
    leading_in: 0.12,
    gap_in: 0.04,
};
const FIELD_CARD_BOX: CardBox = CardBox {
    size_pt: 7.0,
    height_in: ROW_HEIGHT_IN,
};
const FULL_CARD: CardGeom = CardGeom {
    scale: CARD_FULL_SCALE,
    guard: CARD_FULL_GUARD_IN,
};
const COMPACT_CARD: CardGeom = CardGeom {
    scale: CARD_COMPACT_SCALE,
    guard: CARD_COMPACT_GUARD_IN,
};

/// Build the report's pages from *payload* and its section *plan*.
///
/// # Errors
///
/// Return [`PdfError::Logo`] when *logo* is set but is not a PNG.
pub fn report_pages(
    payload: &RacePayload,
    sections: &Sections,
    letter: bool,
    logo: Option<&[u8]>,
) -> Result<Vec<Page>, PdfError> {
    let logo = validate_logo(logo)?;
    let running_title = format!("{}{RUNNING_TITLE_SEP}", payload.event.title);
    let footer = Footer {
        credits: footer_credits(payload),
        generated: payload.event.generated.clone(),
        numbered: true,
    };
    let canvas = Canvas::new(page_size(letter), Some(running_title), footer);
    let mut report = Report {
        canvas,
        payload,
        plan: sections,
        opts: payload.options,
    };
    report.build(logo.as_deref());
    Ok(report.canvas.into_pages())
}

/// Build the one-page podium poster's pages from *payload*.
///
/// # Errors
///
/// Return [`PdfError::Logo`] when *logo* is set but is not a PNG.
pub fn poster_pages(
    payload: &RacePayload,
    letter: bool,
    logo: Option<&[u8]>,
) -> Result<Vec<Page>, PdfError> {
    let logo = validate_logo(logo)?;
    let footer = Footer {
        credits: footer_credits(payload),
        generated: payload.event.generated.clone(),
        numbered: false,
    };
    let canvas = Canvas::new(page_size(letter), None, footer);
    let mut poster = Poster { canvas, payload };
    poster.build(logo.as_deref());
    Ok(poster.canvas.into_pages())
}

/// Return *logo* as owned PNG bytes, or reject it.
fn validate_logo(logo: Option<&[u8]>) -> Result<Option<Vec<u8>>, PdfError> {
    match logo {
        None => Ok(None),
        Some(bytes) if bytes.starts_with(&PNG_SIGNATURE) => Ok(Some(bytes.to_vec())),
        Some(_) => Err(PdfError::Logo("the organizer logo is not a PNG".to_owned())),
    }
}

/// Return the footer's credit line from *payload*.
fn footer_credits(payload: &RacePayload) -> String {
    format!("{} · {}", payload.event.organizer, payload.event.scorer)
}

/// Return the comma-grouped spelling of *value*.
fn grouped(value: &impl std::fmt::Display) -> String {
    group_digits(&value.to_string())
}

/// Insert a comma every three digits of *digits*.
fn group_digits(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len());
    let len = digits.len();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (len - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Return *text* with its first character upper-cased.
fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    let first = chars
        .next()
        .map_or_else(String::new, |ch| ch.to_uppercase().collect::<String>());
    format!("{first}{}", chars.as_str())
}

/// Return the cover's bragging-rights clause for the shown *options*.
fn bragging_clause(options: &ExportOptions) -> String {
    let mut subjects: Vec<&str> = Vec::new();
    if options.laps_board {
        subjects.push("Lap counts");
    }
    if options.show_times {
        subjects.push("times");
    }
    if subjects.is_empty() {
        return String::new();
    }
    let joined = subjects.join(" and ");
    format!(
        " {} below are unofficial and shown for bragging rights only.",
        capitalize_first(&joined)
    )
}

/// Format *`lap_km`* as the caption's whole or fractional spelling.
fn format_km(lap_km: f64) -> String {
    format!("{lap_km}")
}

/// Return one card pair's face text.
fn pair_text(pair: &CardPair) -> String {
    if pair.rank == "JK" {
        JOKER_TEXT.to_owned()
    } else {
        format!("{}{}", pair.rank, suit_glyph(&pair.suit))
    }
}

/// Return the suit letter's glyph, or none for an unknown letter.
fn suit_glyph(letter: &str) -> &'static str {
    match letter {
        "s" => "♠",
        "h" => "♥",
        "d" => "♦",
        "c" => "♣",
        _ => "",
    }
}

/// Return the face colour for one pair's suit letter.
fn card_color(suit: &str) -> Color {
    match suit {
        "h" | "d" => SUIT_RED,
        "j" => STEEL,
        _ => INK,
    }
}

/// Return a row's drawn tie-break marker, or the empty string.
fn draw_marker(row: &ResultRow) -> String {
    if row.dns {
        return String::new();
    }
    let Some(draw) = row.draw.as_ref() else {
        return String::new();
    };
    let card = pair_text(draw);
    if row.hand.is_empty() {
        format!("DRAW {card}")
    } else {
        format!(" · DRAW {card}")
    }
}

/// Return a row's Laps cell.
fn laps_cell(row: &ResultRow) -> String {
    if row.dns {
        "DNS".to_owned()
    } else {
        row.laps.to_string()
    }
}

/// Return a row's time cell, blank for a DNS row or a missing reading.
fn time_cell<'a>(row: &ResultRow, value: Option<&'a str>) -> &'a str {
    if row.dns { "" } else { value.unwrap_or("") }
}

/// Return a row's place cell, blank for a DNS row.
fn place_text(row: &ResultRow) -> String {
    if row.dns {
        String::new()
    } else {
        row.place.to_string()
    }
}

/// Return whether *row* names a team; a display suffix is allowed.
fn is_team_row(row: &ResultRow) -> bool {
    row.entry_type.to_uppercase().starts_with("TEAM")
}

/// Return the poster card's name line, plate-less for a team.
fn poster_name(row: &ResultRow) -> String {
    if is_team_row(row) {
        row.entry.clone()
    } else {
        format!("#{} {}", row.plate, row.entry)
    }
}

/// Return the poster card's kind, sex and laps line.
fn poster_subtitle(row: &ResultRow) -> String {
    let kind = if is_team_row(row) { "Team" } else { "Solo" };
    let sex = row
        .sex
        .as_ref()
        .map_or_else(String::new, |sex| format!(" ({sex})"));
    format!("{kind}{sex} — {} · {} laps", row.entry, row.laps)
}

/// Return the whole-hand sub-row's text.
fn drawn_text(cards: &[CardPair]) -> String {
    let run = cards.iter().map(pair_text).collect::<Vec<_>>().join(" ");
    format!("All {} cards, in draw order: {run}", cards.len())
}

/// Wrap *text* to *`max_width_pt`*, breaking at spaces only.
fn wrap(face: FontFace, size_pt: f32, text: &str, max_width_pt: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }
        let candidate = text_width(face, &current, size_pt)
            + text_width(face, " ", size_pt)
            + text_width(face, word, size_pt);
        if candidate > max_width_pt {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Return the x of column *index*.
fn column_x(widths: &[f32], index: usize) -> f32 {
    MARGIN_IN + widths.iter().take(index).sum::<f32>()
}

/// Return the width of column *index*.
fn column_width(widths: &[f32], index: usize) -> f32 {
    widths.get(index).copied().unwrap_or(0.0)
}

/// Return column *index*'s span.
fn column_span(widths: &[f32], index: usize) -> Span {
    Span {
        x: column_x(widths, index),
        w: column_width(widths, index),
    }
}

/// Build one text operation in points.
fn text_op(at: Pos, style: TextStyle, text: &str) -> Op {
    Op::Text {
        x: at.x * POINTS_PER_INCH,
        y: at.y * POINTS_PER_INCH,
        font: style.font,
        size: style.size,
        color: style.color,
        text: text.to_owned(),
    }
}

/// Build one line operation in points.
fn line_op(from: Pos, to: Pos, stroke: Stroke) -> Op {
    Op::Line {
        x1: from.x * POINTS_PER_INCH,
        y1: from.y * POINTS_PER_INCH,
        x2: to.x * POINTS_PER_INCH,
        y2: to.y * POINTS_PER_INCH,
        color: stroke.color,
        width_pt: stroke.width_pt,
    }
}

/// Return the four registration ticks at one margin-box corner pair.
fn registration_marks(page: &Page, top: bool) -> Vec<Op> {
    let width_in = page.width_pt / POINTS_PER_INCH;
    let height_in = page.height_pt / POINTS_PER_INCH;
    let left = MARGIN_IN;
    let right = width_in - MARGIN_IN;
    let y = if top {
        MARGIN_IN
    } else {
        height_in - MARGIN_IN
    };
    let tick = REGISTRATION_TICK_IN;
    let vertical = if top { -tick } else { tick };
    let stroke = Stroke {
        color: INK,
        width_pt: REGISTRATION_LINE_PT,
    };
    vec![
        line_op(Pos { x: left, y }, Pos { x: left - tick, y }, stroke),
        line_op(
            Pos { x: left, y },
            Pos {
                x: left,
                y: y + vertical,
            },
            stroke,
        ),
        line_op(Pos { x: right, y }, Pos { x: right + tick, y }, stroke),
        line_op(
            Pos { x: right, y },
            Pos {
                x: right,
                y: y + vertical,
            },
            stroke,
        ),
    ]
}

/// Return one page's header: the corner marks and the running title.
fn header_ops(page: &Page, page_index: usize, running_title: Option<&str>) -> Vec<Op> {
    let mut ops = registration_marks(page, true);
    if let Some(title) = running_title.filter(|_| page_index > 0) {
        ops.push(text_op(
            Pos {
                x: MARGIN_IN,
                y: 0.30,
            },
            RUNNING_TITLE,
            title,
        ));
    }
    ops
}

/// Return one page's footer: the corner marks, rule, credits and stamp.
fn footer_ops(page: &Page, paging: Paging, footer: &Footer) -> Vec<Op> {
    let width_in = page.width_pt / POINTS_PER_INCH;
    let height_in = page.height_pt / POINTS_PER_INCH;
    let mut ops = registration_marks(page, false);
    let y = height_in - MARGIN_IN;
    let right = width_in - MARGIN_IN;
    ops.push(line_op(
        Pos { x: MARGIN_IN, y },
        Pos { x: right, y },
        Stroke {
            color: INK,
            width_pt: FOOTER_RULE_LINE_PT,
        },
    ));
    ops.push(text_op(
        Pos {
            x: MARGIN_IN,
            y: y + 0.07,
        },
        FOOTER_TEXT,
        &footer.credits,
    ));
    let page_line = if footer.numbered {
        format!(
            "Page {} of {} · {} · RiverCrossing",
            paging.number, paging.total, footer.generated
        )
    } else {
        format!("{} · RiverCrossing", footer.generated)
    };
    ops.push(text_op(
        Pos {
            x: MARGIN_IN,
            y: y + 0.20,
        },
        FOOTER_TEXT,
        &page_line,
    ));
    ops
}

/// A row's hand prose, cased for the surface that draws it.
struct HandText<'a> {
    row: &'a ResultRow,
    prose: String,
}

impl<'a> HandText<'a> {
    /// Return the report's upper-cased prose.
    fn report(row: &'a ResultRow) -> Self {
        Self {
            row,
            prose: row.hand.to_uppercase(),
        }
    }

    /// Return the poster's title-case prose.
    fn poster(row: &'a ResultRow) -> Self {
        Self {
            row,
            prose: row.hand.clone(),
        }
    }
}

/// The drawing cursor and the pages it has filled.
struct Canvas {
    current: Page,
    finished: Vec<Page>,
    x_in: f32,
    y_in: f32,
    width_pt: f32,
    height_pt: f32,
    running_title: Option<String>,
    footer: Footer,
}

impl Canvas {
    /// Open a canvas on its first page.
    fn new(size: PageSize, running_title: Option<String>, footer: Footer) -> Self {
        let mut current = Page::empty(size.width_pt, size.height_pt);
        let header = header_ops(&current, 0, running_title.as_deref());
        current.ops.extend(header);
        Self {
            current,
            finished: Vec::new(),
            x_in: MARGIN_IN,
            y_in: MARGIN_IN,
            width_pt: size.width_pt,
            height_pt: size.height_pt,
            running_title,
            footer,
        }
    }

    /// Return the printable width, in inches.
    fn content_width_in(&self) -> f32 {
        content_width_in(self.width_pt)
    }

    /// Return the content's right edge, in inches.
    fn right_in(&self) -> f32 {
        MARGIN_IN + self.content_width_in()
    }

    /// Return the content's bottom edge, in inches.
    fn bottom_in(&self) -> f32 {
        self.height_pt / POINTS_PER_INCH - FOOTER_GAP_IN
    }

    /// Begin a new page and reset the cursor to the top margin.
    fn new_page(&mut self) {
        let done = std::mem::replace(
            &mut self.current,
            Page::empty(self.width_pt, self.height_pt),
        );
        self.finished.push(done);
        let index = self.finished.len();
        let mut page = Page::empty(self.width_pt, self.height_pt);
        let header = header_ops(&page, index, self.running_title.as_deref());
        page.ops.extend(header);
        self.current = page;
        self.x_in = MARGIN_IN;
        self.y_in = MARGIN_IN;
    }

    /// Add *op* to the current page.
    fn push(&mut self, op: Op) {
        self.current.ops.push(op);
    }

    /// Advance the cursor by *dy* inches, as the oracle's `ln` does.
    fn advance(&mut self, dy: f32) {
        self.y_in += dy;
        self.x_in = MARGIN_IN;
    }

    /// Add a page when *guard* inches would cross the break.
    fn maybe_break(&mut self, guard: f32) {
        if self.y_in + guard > self.bottom_in() {
            self.new_page();
        }
    }

    /// Return a full-width cell rectangle of *`height_in`* at the cursor.
    fn full_cell_rect(&self, height_in: f32) -> Rect {
        Rect {
            x: MARGIN_IN,
            y: self.y_in,
            w: self.content_width_in(),
            h: height_in,
        }
    }

    /// Draw one aligned text cell.
    fn cell(&mut self, rect: Rect, style: TextStyle, text: &str) {
        if text.is_empty() {
            return;
        }
        let width_in = text_width(style.font, text, style.size) / POINTS_PER_INCH;
        let x = match style.align {
            Align::Left => rect.x,
            Align::Right => rect.x + rect.w - width_in,
            Align::Center => rect.x + (rect.w - width_in) / 2.0,
        };
        self.push(text_op(Pos { x, y: rect.y }, style, text));
    }

    /// Draw one wrapped paragraph and advance past it.
    fn multi_text(&mut self, span: Span, lead: Lead, text: &str) {
        let max_width_pt = span.w * POINTS_PER_INCH;
        let lines = wrap(lead.style.font, lead.style.size, text, max_width_pt);
        let mut y = self.y_in;
        for line in &lines {
            self.push(text_op(Pos { x: span.x, y }, lead.style, line));
            y += lead.leading_in;
        }
        self.y_in = y;
        self.x_in = MARGIN_IN;
    }

    /// Draw one line.
    fn line(&mut self, seg: Seg, stroke: Stroke) {
        self.push(line_op(
            Pos {
                x: seg.x1,
                y: seg.y1,
            },
            Pos {
                x: seg.x2,
                y: seg.y2,
            },
            stroke,
        ));
    }

    /// Draw one filled rectangle.
    fn filled_rect(&mut self, rect: Rect, fill: Color) {
        self.push(Op::FilledRect {
            x: rect.x * POINTS_PER_INCH,
            y: rect.y * POINTS_PER_INCH,
            w: rect.w * POINTS_PER_INCH,
            h: rect.h * POINTS_PER_INCH,
            fill,
        });
    }

    /// Draw the four hairlines around one card face.
    fn card_border(&mut self, rect: Rect) {
        let stroke = Stroke {
            color: CARD_BOX_BORDER,
            width_pt: CARD_BOX_LINE_PT,
        };
        let right = rect.x + rect.w;
        let bottom = rect.y + rect.h;
        self.line(
            Seg {
                x1: rect.x,
                y1: rect.y,
                x2: right,
                y2: rect.y,
            },
            stroke,
        );
        self.line(
            Seg {
                x1: rect.x,
                y1: bottom,
                x2: right,
                y2: bottom,
            },
            stroke,
        );
        self.line(
            Seg {
                x1: rect.x,
                y1: rect.y,
                x2: rect.x,
                y2: bottom,
            },
            stroke,
        );
        self.line(
            Seg {
                x1: right,
                y1: rect.y,
                x2: right,
                y2: bottom,
            },
            stroke,
        );
    }

    /// Draw a section rule across the content at the cursor.
    fn rule(&mut self) {
        let y = self.y_in;
        let right = self.right_in();
        self.line(
            Seg {
                x1: MARGIN_IN,
                y1: y,
                x2: right,
                y2: y,
            },
            Stroke {
                color: INK,
                width_pt: RULE_LINE_PT,
            },
        );
    }

    /// Finish one table row with its light separator rule.
    fn row_rule(&mut self) {
        self.advance(ROW_HEIGHT_IN);
        let y = self.y_in;
        let right = self.right_in();
        self.line(
            Seg {
                x1: MARGIN_IN,
                y1: y,
                x2: right,
                y2: y,
            },
            Stroke {
                color: INK,
                width_pt: ROW_RULE_LINE_PT,
            },
        );
        self.advance(0.05);
    }

    /// Draw a section heading, breaking first when it would not fit.
    fn section_heading(&mut self, text: &str) {
        self.maybe_break(HEADING_GUARD_IN);
        self.advance(0.06);
        let rect = Rect {
            x: MARGIN_IN,
            y: self.y_in,
            w: self.content_width_in(),
            h: 0.22,
        };
        self.cell(rect, SECTION_HEAD, text);
        self.advance(0.28);
    }

    /// Draw a full-field subsection head and its rule.
    fn subsection_heading(&mut self, text: &str) {
        self.maybe_break(HEADING_GUARD_IN);
        self.advance(0.04);
        let rect = Rect {
            x: MARGIN_IN,
            y: self.y_in,
            w: self.content_width_in(),
            h: 0.20,
        };
        self.cell(rect, SUBSECTION_HEAD, text);
        self.advance(0.24);
        self.rule();
        self.advance(0.03);
    }

    /// Draw one table header row and the rule under it.
    fn table_header(&mut self, widths: &[f32], labels: &[&str]) {
        let y = self.y_in;
        let mut x = MARGIN_IN;
        for (width, label) in widths.iter().zip(labels.iter()) {
            self.cell(
                Rect {
                    x,
                    y,
                    w: *width,
                    h: 0.16,
                },
                TABLE_HEAD,
                label,
            );
            x += *width;
        }
        self.advance(0.18);
        self.rule();
        self.advance(0.03);
    }

    /// Draw one boxed card run and return the cursor after it.
    fn boxed_cards(&mut self, cards: &[CardPair], span: Span, card: CardBox) -> f32 {
        let right = span.x + span.w;
        let y = self.y_in;
        let mut x = span.x;
        for face in cards {
            let text = pair_text(face);
            let width = text_width(FontFace::Glyph, &text, card.size_pt) / POINTS_PER_INCH
                + 2.0 * CARD_PAD_IN;
            if x + width > right {
                break;
            }
            let rect = Rect {
                x,
                y,
                w: width,
                h: card.height_in,
            };
            self.filled_rect(rect, CARD_BOX_FILL);
            self.card_border(rect);
            self.cell(
                rect,
                TextStyle {
                    font: FontFace::Glyph,
                    size: card.size_pt,
                    color: card_color(&face.suit),
                    align: Align::Center,
                },
                &text,
            );
            x += width + CARD_GAP_IN;
        }
        x
    }

    /// Draw a hand prose cell and, after it, its drawn-card marker.
    fn hand_cell(&mut self, hand: &HandText<'_>, span: Span, style: TextStyle) {
        let row = hand.row;
        let marker = draw_marker(row);
        let y = self.y_in;
        if marker.is_empty() {
            self.cell(
                Rect {
                    x: span.x,
                    y,
                    w: span.w,
                    h: 0.0,
                },
                style,
                &hand.prose,
            );
            return;
        }
        let marker_style = TextStyle {
            font: FontFace::Glyph,
            size: style.size,
            color: marker_color(row),
            align: Align::Left,
        };
        let marker_width = text_width(FontFace::Glyph, &marker, style.size) / POINTS_PER_INCH;
        let prose_width = text_width(style.font, &hand.prose, style.size) / POINTS_PER_INCH;
        let hand_width = prose_width.min((span.w - marker_width).max(0.0));
        self.cell(
            Rect {
                x: span.x,
                y,
                w: hand_width,
                h: 0.0,
            },
            style,
            &hand.prose,
        );
        self.cell(
            Rect {
                x: span.x + hand_width,
                y,
                w: marker_width,
                h: 0.0,
            },
            marker_style,
            &marker,
        );
    }

    /// Draw the muted whole-hand sub-row at *`indent_in`*.
    fn drawn_row(&mut self, cards: &[CardPair], indent_in: f32, drawn: DrawnRow) {
        let style = TextStyle {
            font: FontFace::Glyph,
            size: drawn.size_pt,
            color: INK,
            align: Align::Left,
        };
        let span = Span {
            x: MARGIN_IN + indent_in,
            w: self.content_width_in() - indent_in,
        };
        let lead = Lead {
            style,
            leading_in: drawn.leading_in,
        };
        let text = drawn_text(cards);
        self.multi_text(span, lead, &text);
        self.advance(drawn.gap_in);
    }

    /// Draw the organizer logo at the top right. The height is `0.0` so the
    /// writer keeps the PNG's own aspect ratio from the drawn width.
    fn logo(&mut self, png: &[u8]) {
        let width_in = COVER_LOGO_WIDTH_IN;
        let x = self.width_pt / POINTS_PER_INCH - MARGIN_IN - width_in;
        self.push(Op::Image {
            x: x * POINTS_PER_INCH,
            y: MARGIN_IN * POINTS_PER_INCH,
            w: width_in * POINTS_PER_INCH,
            h: 0.0,
            png: png.to_vec(),
        });
    }

    /// Finish the canvas: add every page's footer and return the pages.
    fn into_pages(mut self) -> Vec<Page> {
        let done = std::mem::replace(
            &mut self.current,
            Page::empty(self.width_pt, self.height_pt),
        );
        self.finished.push(done);
        let total = self.finished.len();
        for (index, page) in self.finished.iter_mut().enumerate() {
            let paging = Paging {
                number: index + 1,
                total,
            };
            let ops = footer_ops(page, paging, &self.footer);
            page.ops.extend(ops);
        }
        self.finished
    }
}

/// Return the marker's colour: the drawn card's own face.
fn marker_color(row: &ResultRow) -> Color {
    row.draw
        .as_ref()
        .map_or(STEEL, |pair| card_color(&pair.suit))
}

/// The report document's drawing state.
struct Report<'a> {
    canvas: Canvas,
    payload: &'a RacePayload,
    plan: &'a Sections,
    opts: ExportOptions,
}

impl Report<'_> {
    /// Draw every report section in the oracle's order.
    fn build(&mut self, logo: Option<&[u8]>) {
        self.cover(logo);
        self.podiums();
        self.top_lists();
        self.laps_boards();
        self.time_board();
        if self.opts.full_field {
            self.full_field();
        }
    }

    /// Draw the page-1 cover.
    #[expect(
        clippy::too_many_lines,
        reason = "the cover reproduces seven oracle header blocks in one ordered flow"
    )]
    fn cover(&mut self, logo: Option<&[u8]>) {
        if let Some(png) = logo {
            self.canvas.logo(png);
            self.canvas.advance(COVER_LOGO_HEADER_RESERVE_IN);
        }
        let event = &self.payload.event;
        let kicker = self.canvas.full_cell_rect(0.16);
        self.canvas.cell(kicker, COVER_KICKER, &event.kicker);
        self.canvas.advance(0.20);
        let title = self.canvas.full_cell_rect(0.40);
        self.canvas.cell(title, COVER_TITLE, &event.title);
        self.canvas.advance(0.44);
        let meta = self.canvas.full_cell_rect(0.16);
        self.canvas.cell(meta, COVER_META, &event.meta);
        self.canvas.advance(0.18);
        let counters = format!(
            "{} · {} · {} · {}",
            grouped(&event.entries),
            grouped(&event.laps),
            grouped(&event.cards),
            grouped(&event.riders)
        );
        let counter_cell = self.canvas.full_cell_rect(0.16);
        self.canvas.cell(counter_cell, COVER_COUNTERS, &counters);
        self.canvas.advance(0.17);
        let labels = self.canvas.full_cell_rect(0.13);
        self.canvas.cell(
            labels,
            COVER_LABELS,
            "entries · laps · cards dealt · unique riders",
        );
        self.canvas.advance(0.05);
        self.canvas.rule();
        self.canvas.advance(0.10);
        let disclaimer = format!(
            "It's not a race, it's a poker run — placings are by best poker hand.{}",
            bragging_clause(&self.opts)
        );
        let span = Span {
            x: MARGIN_IN,
            w: self.canvas.content_width_in(),
        };
        self.canvas.multi_text(
            span,
            Lead {
                style: DISCLAIMER,
                leading_in: 0.14,
            },
            &disclaimer,
        );
        self.canvas.advance(0.04);
        if let Some(note) = &self.payload.self_test_note {
            self.canvas.multi_text(
                span,
                Lead {
                    style: SELF_TEST,
                    leading_in: 0.14,
                },
                note,
            );
            self.canvas.advance(0.04);
        }
    }

    /// Draw the per-kind "Best hands" podium cards.
    fn podiums(&mut self) {
        if !self.plan.podium_teams.is_empty() {
            self.canvas.section_heading("Best hands — teams");
            for row in &self.plan.podium_teams {
                self.podium_card(row);
            }
        }
        if !self.plan.podium_solo.is_empty() {
            let title = if self.plan.teams.is_empty() {
                "Best hands — top 3"
            } else {
                "Best hands — solo riders"
            };
            self.canvas.section_heading(title);
            for row in &self.plan.podium_solo {
                self.podium_card(row);
            }
        }
    }

    /// Draw one report podium card.
    #[expect(
        clippy::too_many_lines,
        reason = "the card draws its place, name, run, cards, hand and whole hand in order"
    )]
    fn podium_card(&mut self, row: &ResultRow) {
        self.canvas.maybe_break(PODIUM_GUARD_IN);
        let (indent, content) = (0.70, self.canvas.content_width_in());
        let y = self.canvas.y_in;
        let place_rect = Rect {
            x: MARGIN_IN,
            y,
            w: indent,
            h: 0.42,
        };
        self.canvas.cell(
            place_rect,
            report_place_style(row.place),
            &row.place.to_string(),
        );
        let name = poster_name(row);
        let name_rect = Rect {
            x: MARGIN_IN + indent,
            y,
            w: content - indent,
            h: 0.24,
        };
        self.canvas.cell(name_rect, PODIUM_NAME, &name);
        self.canvas.advance(0.26);
        let mut subtitle = format!("{} · {} laps", row.entry_type, row.laps);
        if self.opts.show_times {
            subtitle.push_str(" · ");
            subtitle.push_str(row.total.as_deref().unwrap_or(""));
        }
        let subtitle_rect = Rect {
            x: MARGIN_IN + indent,
            y: self.canvas.y_in,
            w: content - indent,
            h: 0.13,
        };
        self.canvas.cell(subtitle_rect, PODIUM_SUBTITLE, &subtitle);
        self.canvas.advance(0.16);
        let span = Span {
            x: MARGIN_IN + indent,
            w: 2.0,
        };
        self.canvas.boxed_cards(&row.cards, span, FIELD_CARD_BOX);
        self.canvas.advance(0.24);
        let hand_span = Span {
            x: MARGIN_IN + indent,
            w: content - indent,
        };
        self.canvas
            .hand_cell(&HandText::report(row), hand_span, PODIUM_HAND_STYLE);
        self.canvas.advance(0.20);
        if self.opts.all_cards {
            self.canvas.drawn_row(&row.drawn, indent, REPORT_DRAWN);
        }
        self.canvas.advance(0.12);
    }

    /// Draw the standings tables.
    fn top_lists(&mut self) {
        let content = self.canvas.content_width_in();
        if !self.plan.top_teams.is_empty() {
            self.canvas.section_heading("Top teams");
            let widths = team_top_widths(content, self.opts.show_times);
            let labels = self.top_labels(false);
            self.canvas.table_header(&widths, &labels);
            for row in &self.plan.top_teams {
                self.canvas.maybe_break(ROW_GUARD_IN);
                self.standings_row(&widths, row, false);
            }
        }
        if !self.plan.top_solo.is_empty() {
            let title = if self.plan.teams.is_empty() {
                "Top ten"
            } else {
                "Top solo riders"
            };
            self.canvas.section_heading(title);
            let widths = top_solo_widths(content, self.opts.show_times);
            let labels = self.top_labels(true);
            self.canvas.table_header(&widths, &labels);
            for row in &self.plan.top_solo {
                self.canvas.maybe_break(ROW_GUARD_IN);
                self.standings_row(&widths, row, true);
            }
        }
    }

    /// Return one top-list header's labels.
    fn top_labels(&self, show_plate: bool) -> Vec<&'static str> {
        let mut labels = vec!["Place", "Entry", "Laps"];
        if show_plate {
            labels.insert(1, "Plate");
        }
        if self.opts.show_times {
            labels.push("Total time");
        }
        labels.push("Best 5 cards");
        labels.push("Hand");
        labels
    }

    /// Draw one top-list row.
    fn standings_row(&mut self, widths: &[f32], row: &ResultRow, show_plate: bool) {
        let y = self.canvas.y_in;
        let mut col = 0;
        self.column_cell(widths, Slot { index: col, y }, ROW_BOLD, &place_text(row));
        col += 1;
        if show_plate {
            self.column_cell(
                widths,
                Slot { index: col, y },
                ROW_BOLD,
                &row.plate.to_string(),
            );
            col += 1;
        }
        self.column_cell(widths, Slot { index: col, y }, ROW_STYLE, &row.entry);
        col += 1;
        self.column_cell(widths, Slot { index: col, y }, ROW_RIGHT, &laps_cell(row));
        col += 1;
        if self.opts.show_times {
            self.column_cell(
                widths,
                Slot { index: col, y },
                ROW_STYLE,
                row.total.as_deref().unwrap_or(""),
            );
            col += 1;
        }
        let cards = column_span(widths, col);
        self.canvas.boxed_cards(&row.cards, cards, FIELD_CARD_BOX);
        col += 1;
        let hand = column_span(widths, col);
        self.canvas
            .hand_cell(&HandText::report(row), hand, HAND_STYLE);
        self.canvas.row_rule();
    }

    /// Draw one table cell at *slot*.
    fn column_cell(&mut self, widths: &[f32], slot: Slot, style: TextStyle, text: &str) {
        let rect = Rect {
            x: column_x(widths, slot.index),
            y: slot.y,
            w: column_width(widths, slot.index),
            h: ROW_HEIGHT_IN,
        };
        self.canvas.cell(rect, style, text);
    }

    /// Draw the per-kind "Most laps" boards.
    fn laps_boards(&mut self) {
        if !self.plan.laps_teams.is_empty() {
            self.laps_board("Most laps — teams", &self.plan.laps_teams, false);
        }
        if !self.plan.laps_solo.is_empty() {
            let title = if self.plan.teams.is_empty() {
                "Most laps"
            } else {
                "Most laps — solo riders"
            };
            self.laps_board(title, &self.plan.laps_solo, true);
        }
    }

    /// Draw one most-laps board.
    fn laps_board(&mut self, title: &str, rows: &[LapsBoardRow], show_plate: bool) {
        self.canvas.section_heading(title);
        let caption = format!("Unofficial — {} km per lap.", format_km(self.opts.lap_km));
        let caption_rect = self.canvas.full_cell_rect(0.13);
        self.canvas.cell(caption_rect, CAPTION, &caption);
        self.canvas.advance(0.16);
        let content = self.canvas.content_width_in();
        let widths = if show_plate {
            laps_solo_widths(content, self.opts.show_times)
        } else {
            laps_team_widths(content, self.opts.show_times)
        };
        let labels = self.laps_labels(show_plate);
        self.canvas.table_header(&widths, &labels);
        for (index, row) in rows.iter().enumerate() {
            self.canvas.maybe_break(ROW_GUARD_IN);
            self.laps_row(
                &widths,
                row,
                Placement {
                    place: index + 1,
                    show_plate,
                },
            );
        }
    }

    /// Return one laps-board header's labels.
    fn laps_labels(&self, show_plate: bool) -> Vec<&'static str> {
        let mut labels = vec!["#", "Entry", "Laps"];
        if show_plate {
            labels.insert(1, "Plate");
        }
        if self.opts.show_times {
            labels.push("Total");
        }
        labels
    }

    /// Draw one most-laps row.
    fn laps_row(&mut self, widths: &[f32], row: &LapsBoardRow, placement: Placement) {
        let y = self.canvas.y_in;
        let mut col = 0;
        self.column_cell(
            widths,
            Slot { index: col, y },
            ROW_BOLD,
            &placement.place.to_string(),
        );
        col += 1;
        if placement.show_plate {
            self.column_cell(
                widths,
                Slot { index: col, y },
                ROW_BOLD,
                &format!("#{}", row.plate),
            );
            col += 1;
        }
        self.column_cell(widths, Slot { index: col, y }, ROW_STYLE, &row.entry);
        col += 1;
        self.column_cell(
            widths,
            Slot { index: col, y },
            ROW_RIGHT,
            &row.laps.to_string(),
        );
        col += 1;
        if self.opts.show_times {
            self.column_cell(
                widths,
                Slot { index: col, y },
                ROW_STYLE,
                row.total.as_deref().unwrap_or(""),
            );
        }
        self.canvas.row_rule();
    }

    /// Draw the flat fastest board, top ten.
    fn time_board(&mut self) {
        if self.plan.time_board.is_empty() {
            return;
        }
        self.canvas.section_heading("Fastest — laps then time");
        let caption_rect = self.canvas.full_cell_rect(0.13);
        self.canvas.cell(
            caption_rect,
            CAPTION,
            "Most laps, shortest elapsed to the last crossing.",
        );
        self.canvas.advance(0.16);
        let widths = time_widths(self.canvas.content_width_in());
        self.canvas.table_header(
            &widths,
            &["#", "Plate", "Entry", "Laps", "Total", "Avg lap"],
        );
        for (index, row) in self.plan.time_board.iter().enumerate() {
            self.canvas.maybe_break(ROW_GUARD_IN);
            self.time_row(&widths, row, index + 1);
        }
    }

    /// Draw one fastest board row.
    fn time_row(&mut self, widths: &[f32], row: &TimeBoardRow, place: usize) {
        let y = self.canvas.y_in;
        self.column_cell(widths, Slot { index: 0, y }, ROW_BOLD, &place.to_string());
        let show_plate = !self.plan.team_plates.contains(&row.plate);
        let bib = if show_plate {
            format!("#{}", row.plate)
        } else {
            String::new()
        };
        self.column_cell(widths, Slot { index: 1, y }, ROW_BOLD, &bib);
        self.column_cell(widths, Slot { index: 2, y }, ROW_STYLE, &row.entry);
        self.column_cell(
            widths,
            Slot { index: 3, y },
            ROW_STYLE,
            &format!("{} laps", row.laps),
        );
        self.column_cell(widths, Slot { index: 4, y }, BOARD_TOTAL, &row.total);
        self.column_cell(
            widths,
            Slot { index: 5, y },
            ROW_STYLE,
            &format!("avg {}", row.avg),
        );
        self.canvas.row_rule();
    }

    /// Draw the full-field umbrella, its note and both subsections.
    fn full_field(&mut self) {
        self.canvas.section_heading("Full field");
        let mut note = String::from("Every entry, ordered by hand");
        if !self.plan.teams.is_empty() {
            note.push_str(" — teams ranked against teams, solo riders against solo riders");
        }
        note.push_str(". ★ = joker, shown as the card it played.");
        if self.opts.all_cards {
            note.push_str(" This export includes every card drawn (organizer option).");
        }
        let span = Span {
            x: MARGIN_IN,
            w: self.canvas.content_width_in(),
        };
        self.canvas.multi_text(
            span,
            Lead {
                style: FULL_FIELD_NOTE,
                leading_in: 0.13,
            },
            &note,
        );
        self.canvas.advance(0.05);
        if !self.plan.teams.is_empty() {
            self.teams_field();
        }
        if !self.plan.solo.is_empty() {
            self.solo_field();
        }
    }

    /// Draw the full field's Teams subsection.
    fn teams_field(&mut self) {
        self.canvas.subsection_heading("Teams");
        let widths = team_field_widths(self.canvas.content_width_in(), self.opts.show_times);
        let labels = self.team_field_labels();
        self.canvas.table_header(&widths, &labels);
        for row in &self.plan.teams {
            self.canvas.maybe_break(ROW_GUARD_IN);
            self.team_field_row(&widths, row);
            if self.opts.all_cards {
                self.canvas.drawn_row(&row.drawn, 0.0, REPORT_DRAWN);
            }
        }
    }

    /// Return the team field's header labels.
    fn team_field_labels(&self) -> Vec<&'static str> {
        let mut labels = vec!["Place", "Entry", "Laps"];
        if self.opts.show_times {
            labels.push("Total time");
            labels.push("Best lap");
        }
        labels.push("Cards");
        labels.push("Hand");
        labels
    }

    /// Draw one team full-field row.
    fn team_field_row(&mut self, widths: &[f32], row: &ResultRow) {
        let name = if row.dnf {
            format!("{} DNF", row.entry)
        } else {
            row.entry.clone()
        };
        let y = self.canvas.y_in;
        let mut col = 0;
        self.column_cell(widths, Slot { index: col, y }, FIELD_BOLD, &place_text(row));
        col += 1;
        self.column_cell(widths, Slot { index: col, y }, FIELD_STYLE, &name);
        col += 1;
        self.column_cell(widths, Slot { index: col, y }, FIELD_STYLE, &laps_cell(row));
        col += 1;
        if self.opts.show_times {
            self.column_cell(
                widths,
                Slot { index: col, y },
                FIELD_STYLE,
                time_cell(row, row.total.as_deref()),
            );
            col += 1;
            self.column_cell(
                widths,
                Slot { index: col, y },
                FIELD_STYLE,
                time_cell(row, row.best_lap.as_deref()),
            );
            col += 1;
        }
        let cards = column_span(widths, col);
        self.canvas.boxed_cards(&row.cards, cards, FIELD_CARD_BOX);
        col += 1;
        let hand = column_span(widths, col);
        self.canvas
            .hand_cell(&HandText::report(row), hand, HAND_STYLE);
        self.canvas.row_rule();
    }

    /// Draw the full field's Solo riders subsection.
    fn solo_field(&mut self) {
        self.canvas.subsection_heading("Solo riders");
        let widths = field_widths(self.canvas.content_width_in(), self.opts.show_times);
        let labels = self.solo_field_labels();
        self.canvas.table_header(&widths, &labels);
        for row in &self.plan.solo {
            self.canvas.maybe_break(ROW_GUARD_IN);
            self.field_row(&widths, row);
            if self.opts.all_cards {
                self.canvas.drawn_row(&row.drawn, 0.0, REPORT_DRAWN);
            }
        }
    }

    /// Return the solo field's header labels.
    fn solo_field_labels(&self) -> Vec<&'static str> {
        let mut labels = vec!["Place", "Plate", "Entry", "Type", "Laps"];
        if self.opts.show_times {
            labels.push("Total time");
            labels.push("Best lap");
        }
        labels.push("Best hand");
        labels
    }

    /// Draw one solo full-field row.
    #[expect(
        clippy::too_many_lines,
        reason = "the row draws every field column in the oracle's order before its rule"
    )]
    fn field_row(&mut self, widths: &[f32], row: &ResultRow) {
        let cell_name = row
            .sex
            .as_ref()
            .map_or_else(|| row.entry.clone(), |sex| format!("{} ({sex})", row.entry));
        let name = if row.dnf {
            format!("{cell_name} DNF")
        } else {
            cell_name
        };
        let y = self.canvas.y_in;
        let mut col = 0;
        self.column_cell(widths, Slot { index: col, y }, FIELD_BOLD, &place_text(row));
        col += 1;
        self.column_cell(
            widths,
            Slot { index: col, y },
            FIELD_BOLD,
            &row.plate.to_string(),
        );
        col += 1;
        self.column_cell(widths, Slot { index: col, y }, FIELD_STYLE, &name);
        col += 1;
        self.column_cell(widths, Slot { index: col, y }, FIELD_STYLE, &row.entry_type);
        col += 1;
        self.column_cell(widths, Slot { index: col, y }, FIELD_STYLE, &laps_cell(row));
        col += 1;
        if self.opts.show_times {
            self.column_cell(
                widths,
                Slot { index: col, y },
                FIELD_STYLE,
                time_cell(row, row.total.as_deref()),
            );
            col += 1;
            self.column_cell(
                widths,
                Slot { index: col, y },
                FIELD_STYLE,
                time_cell(row, row.best_lap.as_deref()),
            );
        }
        let last = widths.len() - 1;
        let span = column_span(widths, last);
        let after = self.canvas.boxed_cards(&row.cards, span, FIELD_CARD_BOX);
        let remaining = (MARGIN_IN + self.canvas.content_width_in() - after).max(0.0);
        self.canvas.hand_cell(
            &HandText::report(row),
            Span {
                x: after,
                w: remaining,
            },
            HAND_STYLE,
        );
        self.canvas.row_rule();
    }
}

/// Return the report podium place's style.
fn report_place_style(place: i64) -> TextStyle {
    let color = if place == 1 { DEEP_STEEL } else { STEEL };
    TextStyle {
        font: FontFace::Heading,
        size: 30.0,
        color,
        align: Align::Left,
    }
}

/// Return the poster podium place's style at *scale*.
fn poster_place_style(place: usize, scale: f32) -> TextStyle {
    let color = if place == 1 { DEEP_STEEL } else { STEEL };
    TextStyle {
        font: FontFace::Heading,
        size: 34.0 * scale,
        color,
        align: Align::Left,
    }
}

/// The poster document's drawing state.
struct Poster<'a> {
    canvas: Canvas,
    payload: &'a RacePayload,
}

impl Poster<'_> {
    /// Draw the header block and the per-kind cards.
    fn build(&mut self, logo: Option<&[u8]>) {
        self.header_block(logo);
        let plan = poster_sections(self.payload);
        for (heading, rows) in &plan {
            if rows.is_empty() {
                continue;
            }
            if !heading.is_empty() {
                self.section_head(heading);
            }
            let geom = if heading.is_empty() {
                FULL_CARD
            } else {
                COMPACT_CARD
            };
            self.cards(rows, geom);
        }
    }

    /// Draw the event meta, "Best poker hands", the ride name and the rule.
    fn header_block(&mut self, logo: Option<&[u8]>) {
        if let Some(png) = logo {
            self.canvas.logo(png);
            self.canvas.advance(COVER_LOGO_HEADER_RESERVE_IN);
        }
        let content = self.canvas.content_width_in();
        let meta = self.canvas.full_cell_rect(0.16);
        self.canvas
            .cell(meta, POSTER_META, &self.payload.event.meta);
        self.canvas.advance(0.22);
        let title = self.canvas.full_cell_rect(0.34);
        self.canvas.cell(title, POSTER_TITLE, "Best poker hands");
        self.canvas.advance(0.40);
        let ride = self.canvas.full_cell_rect(0.22);
        self.canvas
            .cell(ride, POSTER_SECTION, &self.payload.event.title);
        self.canvas.advance(0.26);
        self.canvas.rule();
        self.canvas.advance(0.12);
        if let Some(note) = &self.payload.self_test_note {
            self.canvas.multi_text(
                Span {
                    x: MARGIN_IN,
                    w: content,
                },
                Lead {
                    style: SELF_TEST,
                    leading_in: 0.14,
                },
                note,
            );
            self.canvas.advance(0.06);
        }
    }

    /// Draw one poster section head.
    fn section_head(&mut self, text: &str) {
        self.canvas.maybe_break(POSTER_HEADING_GUARD_IN);
        self.canvas.advance(0.04);
        let rect = self.canvas.full_cell_rect(0.22);
        self.canvas.cell(rect, POSTER_SECTION, text);
        self.canvas.advance(0.26);
    }

    /// Draw one section's cards, numbered from one.
    fn cards(&mut self, rows: &[ResultRow], geom: CardGeom) {
        for (index, row) in rows.iter().enumerate() {
            self.podium_card(row, index + 1, geom);
        }
    }

    /// Draw one poster podium card.
    #[expect(
        clippy::too_many_lines,
        reason = "the card draws its five scaled lines and its large card run in order"
    )]
    fn podium_card(&mut self, row: &ResultRow, place: usize, geom: CardGeom) {
        self.canvas.maybe_break(geom.guard);
        let scale = geom.scale;
        let content = self.canvas.content_width_in();
        let place_rect = self.canvas.full_cell_rect(0.5 * scale);
        self.canvas.cell(
            place_rect,
            poster_place_style(place, scale),
            &place.to_string(),
        );
        self.canvas.advance(0.54 * scale);
        let indent = 0.95 * scale;
        let name_rect = Rect {
            x: MARGIN_IN + indent,
            y: self.canvas.y_in,
            w: content - indent,
            h: 0.24 * scale,
        };
        self.canvas
            .cell(name_rect, poster_name_style(scale), &poster_name(row));
        self.canvas.advance(0.28 * scale);
        let subtitle_rect = Rect {
            x: MARGIN_IN + indent,
            y: self.canvas.y_in,
            w: content - indent,
            h: 0.14 * scale,
        };
        self.canvas.cell(
            subtitle_rect,
            poster_subtitle_style(scale),
            &poster_subtitle(row),
        );
        self.canvas.advance(0.18 * scale);
        let span = Span {
            x: MARGIN_IN + indent,
            w: content - indent,
        };
        self.canvas
            .hand_cell(&HandText::poster(row), span, poster_hand_style(scale));
        self.canvas.advance(0.20 * scale);
        if self.payload.options.all_cards {
            self.canvas.drawn_row(
                &row.drawn,
                indent,
                DrawnRow {
                    size_pt: 6.5 * scale,
                    leading_in: 0.12 * scale,
                    gap_in: 0.04 * scale,
                },
            );
        }
        let card_span = Span {
            x: MARGIN_IN + indent,
            w: content - indent,
        };
        self.canvas.boxed_cards(
            &row.cards,
            card_span,
            CardBox {
                size_pt: 18.0 * scale,
                height_in: 0.30 * scale,
            },
        );
        self.canvas.advance(0.32 * scale);
    }
}

/// Return the poster card's name style at *scale*.
fn poster_name_style(scale: f32) -> TextStyle {
    TextStyle {
        font: FontFace::Heading,
        size: 14.0 * scale,
        color: INK,
        align: Align::Left,
    }
}

/// Return the poster card's subtitle style at *scale*.
fn poster_subtitle_style(scale: f32) -> TextStyle {
    TextStyle {
        font: FontFace::Body,
        size: 8.5 * scale,
        color: INK,
        align: Align::Left,
    }
}

/// Return the poster card's hand style at *scale*.
fn poster_hand_style(scale: f32) -> TextStyle {
    TextStyle {
        font: FontFace::Heading,
        size: 10.0 * scale,
        color: STEEL,
        align: Align::Left,
    }
}

#[cfg(test)]
mod tests;
