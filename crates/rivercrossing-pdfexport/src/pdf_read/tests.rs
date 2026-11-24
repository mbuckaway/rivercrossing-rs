// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the test-only PDF scanner, run against the writer's own output.

use pretty_assertions::assert_eq;
use rstest::rstest;
use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

use super::{has_name, has_trailer_key, info_has, media_box_contains, page_count};
use crate::fonts::FontFace;
use crate::geometry::{A4_HEIGHT_PT, A4_WIDTH_PT, INK, LETTER_HEIGHT_PT, LETTER_WIDTH_PT};
use crate::layout::{Op, Page};
use crate::writer;

const TITLE: &str = "Test Poker Run 2026";

fn stamp() -> OffsetDateTime {
    let date = Date::from_calendar_date(2026, Month::September, 20).expect("the fixture date");
    let clock = Time::from_hms(16, 7, 0).expect("the fixture clock");
    OffsetDateTime::new_in_offset(date, clock, UtcOffset::UTC)
}

fn text_page(width_pt: f32, height_pt: f32) -> Page {
    Page {
        width_pt,
        height_pt,
        ops: vec![Op::Text {
            x: 42.0,
            y: 80.0,
            font: FontFace::Body,
            size: 11.0,
            color: INK,
            text: "Entry One".to_owned(),
        }],
    }
}

fn letter_pages(count: usize) -> Vec<Page> {
    (0..count)
        .map(|_| text_page(LETTER_WIDTH_PT, LETTER_HEIGHT_PT))
        .collect()
}

fn written(pages: &[Page]) -> Vec<u8> {
    writer::write(pages, TITLE, stamp()).expect("the fixture pages write")
}

// ---------------------------------------------------------------- page count

#[rstest]
#[case(b"/Type /Page", 1)]
#[case(b"/Type/Page", 1)]
#[case(b"/Type /Pages", 0)]
#[case(b"/TypeX", 0)]
#[case(b"/Type Nope", 0)]
#[case(b"", 0)]
fn test_page_count_of_scan_input_returns_count(#[case] input: &[u8], #[case] expected: usize) {
    // Act
    let count = page_count(input);

    // Assert
    assert_eq!(count, expected);
}

#[test]
fn test_page_count_of_single_page_document_returns_one() {
    // Arrange
    let bytes = written(&letter_pages(1));

    // Act
    let count = page_count(&bytes);

    // Assert
    assert_eq!(count, 1);
}

#[test]
fn test_page_count_of_two_page_document_returns_two() {
    // Arrange
    let bytes = written(&letter_pages(2));

    // Act
    let count = page_count(&bytes);

    // Assert
    assert_eq!(count, 2);
}

// ----------------------------------------------------------------- media box

#[test]
fn test_media_box_contains_letter_size() {
    // Arrange
    let bytes = written(&letter_pages(1));

    // Act
    let contains = media_box_contains(&bytes, LETTER_WIDTH_PT, LETTER_HEIGHT_PT);

    // Assert
    assert!(contains);
}

#[test]
fn test_media_box_contains_a4_size() {
    // Arrange
    let bytes = written(&[text_page(A4_WIDTH_PT, A4_HEIGHT_PT)]);

    // Act
    let contains = media_box_contains(&bytes, A4_WIDTH_PT, A4_HEIGHT_PT);

    // Assert
    assert!(contains);
}

#[test]
fn test_media_box_rejects_other_size() {
    // Arrange
    let bytes = written(&letter_pages(1));

    // Act
    let contains = media_box_contains(&bytes, A4_WIDTH_PT, A4_HEIGHT_PT);

    // Assert
    assert!(!contains);
}

#[rstest]
#[case(b"%PDF-1.7")]
#[case(b"/MediaBox 3")]
#[case(b"/MediaBox [\xff]")]
#[case(b"/MediaBox [0 0 612]")]
#[case(b"/MediaBox []")]
#[case(b"/MediaBox [1 0 612 792]")]
#[case(b"/MediaBox [0 1 612 792]")]
#[case(b"/MediaBox [0 0 613 792]")]
#[case(b"/MediaBox [0 0 612 793]")]
fn test_media_box_contains_of_scan_input_is_false(#[case] input: &[u8]) {
    // Act
    let contains = media_box_contains(input, LETTER_WIDTH_PT, LETTER_HEIGHT_PT);

    // Assert
    assert!(!contains);
}

// -------------------------------------------------------------------- trailer

#[test]
fn test_has_trailer_key_finds_info_entry() {
    // Arrange
    let bytes = written(&letter_pages(1));

    // Act
    let found = has_trailer_key(&bytes, "/Info");

    // Assert
    assert!(found);
}

#[test]
fn test_has_trailer_key_rejects_absent_key() {
    // Arrange
    let bytes = written(&letter_pages(1));

    // Act
    let found = has_trailer_key(&bytes, "/Foo");

    // Assert
    assert!(!found);
}

#[rstest]
#[case(b"")]
#[case(b"%PDF-1.7")]
#[case(b"trailer")]
fn test_has_trailer_key_of_scan_input_is_false(#[case] input: &[u8]) {
    // Act
    let found = has_trailer_key(input, "/Info");

    // Assert
    assert!(!found);
}

// ------------------------------------------------------------- document info

#[test]
fn test_info_has_title_and_both_dates() {
    // Arrange
    let bytes = written(&letter_pages(1));

    // Act
    let title = info_has(&bytes, "/Title");
    let created = info_has(&bytes, "/CreationDate");
    let modified = info_has(&bytes, "/ModDate");

    // Assert
    assert!(title);
    assert!(created);
    assert!(modified);
}

#[test]
fn test_info_has_rejects_document_without_trailer() {
    // Arrange
    let bytes = b"%PDF-1.7";

    // Act
    let has_title = info_has(bytes, "/Title");

    // Assert
    assert!(!has_title);
}

#[rstest]
#[case(b"trailer\n<</Size 1/Root 1 0 R>>")]
#[case(b"trailer\n<</Info R>>")]
#[case(b"trailer\n<</Info 7 0 R>>")]
#[case(b"trailer\n<</Info 7 0 R>>\n7 0 obj<</Title (x)>>")]
fn test_info_has_of_incomplete_document_is_false(#[case] input: &[u8]) {
    // Act
    let has_title = info_has(input, "/Title");

    // Assert
    assert!(!has_title);
}

// ------------------------------------------------------------------- names

#[test]
fn test_has_name_rejects_document_furniture_keys() {
    // Arrange
    let bytes = written(&letter_pages(1));

    // Act
    let creator = has_name(&bytes, "/Creator");
    let producer = has_name(&bytes, "/Producer");
    let subject = has_name(&bytes, "/Subject");

    // Assert
    assert!(!creator);
    assert!(!producer);
    assert!(!subject);
}

#[test]
fn test_has_name_finds_type0_font_with_to_unicode() {
    // Arrange
    let bytes = written(&letter_pages(1));

    // Act
    let type0 = has_name(&bytes, "/Type0");
    let to_unicode = has_name(&bytes, "/ToUnicode");

    // Assert
    assert!(type0);
    assert!(to_unicode);
}

#[rstest]
#[case(b"", "")]
#[case(b"/Types", "/Type")]
#[case(b"no names here", "/Title")]
fn test_has_name_of_scan_input_is_false(#[case] input: &[u8], #[case] name: &str) {
    // Act
    let found = has_name(input, name);

    // Assert
    assert!(!found);
}
