// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the craft-pdf writer adapter.

use pretty_assertions::assert_eq;
use rstest::rstest;
use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

use super::{aspect_height, write};
use crate::error::PdfError;
use crate::fonts::FontFace;
use crate::geometry::{A4, INK, LETTER};
use crate::layout::{Op, Page};
use crate::pdf_read;

/// The 1×1 transparent PNG the HTML exporter falls back to
/// (`rivercrossing_htmlexport::types::TRANSPARENT_PNG`), decoded.
const PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0xcf, 0x50,
    0x0f, 0x00, 0x03, 0x86, 0x01, 0x80, 0x5a, 0x34, 0x7d, 0x6b, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

const TITLE: &str = "Test Poker Run 2026";

fn created_at(offset: UtcOffset) -> OffsetDateTime {
    let date = Date::from_calendar_date(2026, Month::September, 20).expect("the fixture date");
    let clock = Time::from_hms(16, 7, 0).expect("the fixture clock");
    OffsetDateTime::new_in_offset(date, clock, offset)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn text_op(text: &str, font: FontFace) -> Op {
    Op::Text {
        x: 42.0,
        y: 80.0,
        font,
        size: 11.0,
        color: INK,
        text: text.to_owned(),
    }
}

fn report_page() -> Page {
    Page {
        width_pt: LETTER.width_pt,
        height_pt: LETTER.height_pt,
        ops: vec![
            text_op("RiverCrossing", FontFace::Heading),
            text_op("Entry One", FontFace::Body),
            text_op("♠", FontFace::Glyph),
            Op::FilledRect {
                x: 40.0,
                y: 40.0,
                w: 120.0,
                h: 18.0,
                fill: INK,
            },
            Op::Line {
                x1: 40.0,
                y1: 100.0,
                x2: 500.0,
                y2: 100.0,
                color: INK,
                width_pt: 0.5,
            },
        ],
    }
}

fn image_op(w: f32, h: f32) -> Op {
    Op::Image {
        x: 40.0,
        y: 40.0,
        w,
        h,
        png: PIXEL_PNG.to_vec(),
    }
}

fn poster_page() -> Page {
    Page {
        width_pt: A4.width_pt,
        height_pt: A4.height_pt,
        ops: vec![image_op(57.6, 0.0)],
    }
}

// ------------------------------------------------------------- the document

#[test]
fn test_write_report_page_returns_one_page_pdf() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let pages = [report_page()];

    // Act
    let bytes = write(&pages, TITLE, created_at(UtcOffset::UTC))?;

    // Assert
    assert!(bytes.starts_with(b"%PDF"));
    assert_eq!(pdf_read::page_count(&bytes), 1);
    Ok(())
}

#[test]
fn test_write_poster_page_returns_one_page_pdf() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let pages = [poster_page()];

    // Act
    let bytes = write(&pages, TITLE, created_at(UtcOffset::UTC))?;

    // Assert
    assert_eq!(pdf_read::page_count(&bytes), 1);
    Ok(())
}

#[test]
fn test_write_two_page_document_returns_two_pages() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let pages = [report_page(), poster_page()];

    // Act
    let bytes = write(&pages, TITLE, created_at(UtcOffset::UTC))?;

    // Assert
    assert_eq!(pdf_read::page_count(&bytes), 2);
    Ok(())
}

#[test]
fn test_write_same_input_twice_returns_identical_bytes() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let pages = [report_page()];

    // Act
    let first = write(&pages, TITLE, created_at(UtcOffset::UTC))?;
    let second = write(&pages, TITLE, created_at(UtcOffset::UTC))?;

    // Assert
    assert_eq!(first, second);
    Ok(())
}

// ------------------------------------------------------------------ refusals

#[test]
fn test_write_zero_width_page_returns_layout_error() {
    // Arrange
    let page = Page {
        width_pt: 0.0,
        height_pt: LETTER.height_pt,
        ops: Vec::new(),
    };

    // Act
    let result = write(&[page], TITLE, created_at(UtcOffset::UTC));

    // Assert
    assert!(matches!(result, Err(PdfError::Layout(_))));
}

#[test]
fn test_write_rect_with_non_finite_edge_returns_layout_error() {
    // Arrange
    let page = Page {
        width_pt: LETTER.width_pt,
        height_pt: LETTER.height_pt,
        ops: vec![Op::FilledRect {
            x: 40.0,
            y: 40.0,
            w: f32::NAN,
            h: 18.0,
            fill: INK,
        }],
    };

    // Act
    let result = write(&[page], TITLE, created_at(UtcOffset::UTC));

    // Assert
    assert!(matches!(result, Err(PdfError::Layout(_))));
}

#[test]
fn test_write_line_with_non_finite_endpoint_returns_layout_error() {
    // Arrange
    let page = Page {
        width_pt: LETTER.width_pt,
        height_pt: LETTER.height_pt,
        ops: vec![Op::Line {
            x1: 40.0,
            y1: 100.0,
            x2: f32::INFINITY,
            y2: 100.0,
            color: INK,
            width_pt: 0.5,
        }],
    };

    // Act
    let result = write(&[page], TITLE, created_at(UtcOffset::UTC));

    // Assert
    assert!(matches!(result, Err(PdfError::Layout(_))));
}

#[test]
fn test_write_invalid_png_returns_logo_error() {
    // Arrange
    let page = Page {
        width_pt: LETTER.width_pt,
        height_pt: LETTER.height_pt,
        ops: vec![Op::Image {
            x: 40.0,
            y: 40.0,
            w: 57.6,
            h: 0.0,
            png: b"not a png".to_vec(),
        }],
    };

    // Act
    let result = write(&[page], TITLE, created_at(UtcOffset::UTC));

    // Assert
    assert!(matches!(result, Err(PdfError::Logo(_))));
}

#[test]
fn test_write_image_with_zero_width_returns_layout_error() {
    // Arrange
    let page = Page {
        width_pt: LETTER.width_pt,
        height_pt: LETTER.height_pt,
        ops: vec![image_op(0.0, 10.0)],
    };

    // Act
    let result = write(&[page], TITLE, created_at(UtcOffset::UTC));

    // Assert
    assert!(matches!(result, Err(PdfError::Layout(_))));
}

// --------------------------------------------------------------------- images

#[test]
fn test_write_image_with_zero_height_keeps_png_aspect() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let pages = [poster_page()];

    // Act
    let bytes = write(&pages, TITLE, created_at(UtcOffset::UTC))?;

    // Assert
    assert_eq!(pdf_read::page_count(&bytes), 1);
    Ok(())
}

#[test]
fn test_write_image_with_explicit_height_returns_one_page_pdf()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let page = Page {
        width_pt: LETTER.width_pt,
        height_pt: LETTER.height_pt,
        ops: vec![image_op(40.0, 20.0)],
    };

    // Act
    let bytes = write(&[page], TITLE, created_at(UtcOffset::UTC))?;

    // Assert
    assert_eq!(pdf_read::page_count(&bytes), 1);
    Ok(())
}

#[test]
fn test_aspect_height_wide_png_scales_height_down() {
    // Arrange
    let width_pt = 100.0;
    let pixel_w = 2;
    let pixel_h = 1;

    // Act
    let height = aspect_height(width_pt, pixel_w, pixel_h);

    // Assert
    assert!((height - 50.0).abs() < f32::EPSILON, "got {height}");
}

#[test]
fn test_aspect_height_tall_png_scales_height_up() {
    // Arrange
    let width_pt = 100.0;
    let pixel_w = 1;
    let pixel_h = 2;

    // Act
    let height = aspect_height(width_pt, pixel_w, pixel_h);

    // Assert
    assert!((height - 200.0).abs() < f32::EPSILON, "got {height}");
}

// ------------------------------------------------------------------- metadata

#[rstest]
#[case(-5, -30, "(D:20260920160700-05'30)")]
#[case(2, 0, "(D:20260920160700+02'00)")]
#[case(0, 0, "(D:20260920160700Z)")]
fn test_write_creation_date_keeps_the_callers_offset(
    #[case] hours: i8,
    #[case] minutes: i8,
    #[case] expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let offset = UtcOffset::from_hms(hours, minutes, 0)?;
    let pages = [report_page()];
    let stamp = created_at(offset);

    // Act
    let bytes = write(&pages, TITLE, stamp)?;

    // Assert
    assert!(contains(&bytes, expected.as_bytes()));
    Ok(())
}

#[test]
fn test_write_sets_the_document_title() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let pages = [report_page()];

    // Act
    let bytes = write(&pages, TITLE, created_at(UtcOffset::UTC))?;

    // Assert
    assert!(contains(&bytes, TITLE.as_bytes()));
    Ok(())
}

#[test]
fn test_write_creation_date_before_year_one_writes_year_zero()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let date = Date::from_calendar_date(-1, Month::January, 1)?;
    let clock = Time::from_hms(0, 0, 0)?;
    let stamp = OffsetDateTime::new_in_offset(date, clock, UtcOffset::UTC);
    let pages = [report_page()];

    // Act
    let bytes = write(&pages, TITLE, stamp)?;

    // Assert
    assert!(contains(&bytes, b"(D:00000101000000Z)"));
    Ok(())
}
