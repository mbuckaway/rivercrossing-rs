// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the podium-poster entry point.
//!
//! The content checks read the pure page model, because the writer compresses
//! the content streams; the byte checks read the written PDF through
//! [`crate::pdf_read`].

use pretty_assertions::assert_eq;
use rivercrossing_htmlexport::payload::build_payload;
use tempfile::TempDir;

use super::podium_poster;
use crate::error::PdfError;
use crate::fixtures;
use crate::geometry::{A4_HEIGHT_PT, A4_WIDTH_PT, LETTER_HEIGHT_PT, LETTER_WIDTH_PT};
use crate::layout::{Op, Page, poster_pages};
use crate::pdf_read;
use crate::report::PdfInput;

/// Return the mixed golden input with a logo that is not a PNG.
fn input_with_bad_logo() -> PdfInput {
    let mut input = fixtures::golden_input();
    input.logo_png = Some(b"not a png".to_vec());
    input
}

/// Return the mixed golden input with a plate that is not numeric.
fn input_with_bad_plate() -> PdfInput {
    let mut input = fixtures::golden_input();
    for row in &mut input.export.placed {
        row.result.plate = "not-a-number".to_owned();
    }
    input
}

/// Return one operation's text, when it draws text.
fn text_of(op: &Op) -> Option<&str> {
    match op {
        Op::Text { text, .. } => Some(text.as_str()),
        _ => None,
    }
}

/// Return every text run of *pages*, in drawing order.
fn texts(pages: &[Page]) -> Vec<&str> {
    pages
        .iter()
        .flat_map(page_ops)
        .filter_map(text_of)
        .collect()
}

/// Return one page's operations, or none.
fn page_ops(page: &Page) -> &[Op] {
    &page.ops
}

/// Return whether *pages* draws the exact text *needle*.
fn has_text(pages: &[Page], needle: &str) -> bool {
    texts(pages).into_iter().any(|text| text == needle)
}

/// Return whether *pages* draws a text run that ends with *name*.
fn has_name(pages: &[Page], name: &str) -> bool {
    texts(pages).into_iter().any(|text| text.ends_with(name))
}

/// Return the first drawing index of *needle*, or `usize::MAX` when absent.
fn index_of(pages: &[Page], needle: &str) -> usize {
    texts(pages)
        .into_iter()
        .position(|text| text == needle)
        .unwrap_or(usize::MAX)
}

// ------------------------------------------------------------- the entry point

#[test]
fn test_podium_poster_mixed_field_writes_one_page() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("poster.pdf");
    let input = fixtures::golden_input();

    // Act
    podium_poster(&input, &path)?;
    let bytes = std::fs::read(&path)?;

    // Assert
    assert_eq!(pdf_read::page_count(&bytes), 1);
    Ok(())
}

#[test]
fn test_podium_poster_solo_field_writes_one_page() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("poster.pdf");
    let input = fixtures::solo_input();

    // Act
    podium_poster(&input, &path)?;
    let bytes = std::fs::read(&path)?;

    // Assert
    assert_eq!(pdf_read::page_count(&bytes), 1);
    Ok(())
}

#[test]
fn test_podium_poster_letter_false_writes_a4_page() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("poster.pdf");
    let mut input = fixtures::golden_input();
    input.letter = false;

    // Act
    podium_poster(&input, &path)?;
    let bytes = std::fs::read(&path)?;

    // Assert
    assert!(pdf_read::media_box_contains(
        &bytes,
        A4_WIDTH_PT,
        A4_HEIGHT_PT
    ));
    assert!(!pdf_read::media_box_contains(
        &bytes,
        LETTER_WIDTH_PT,
        LETTER_HEIGHT_PT
    ));
    Ok(())
}

#[test]
fn test_podium_poster_without_logo_embeds_no_image() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("poster.pdf");
    let input = fixtures::golden_input();

    // Act
    podium_poster(&input, &path)?;
    let bytes = std::fs::read(&path)?;

    // Assert
    assert!(!pdf_read::has_name(&bytes, "/Image"));
    Ok(())
}

#[test]
fn test_podium_poster_with_non_png_logo_returns_logo_error() {
    // Arrange
    let dir = TempDir::new().expect("the fixture directory");
    let path = dir.path().join("poster.pdf");
    let input = input_with_bad_logo();

    // Act
    let result = podium_poster(&input, &path);

    // Assert
    assert!(matches!(result, Err(PdfError::Logo(_))));
}

#[test]
fn test_podium_poster_with_non_numeric_plate_returns_layout_error() {
    // Arrange
    let dir = TempDir::new().expect("the fixture directory");
    let path = dir.path().join("poster.pdf");
    let input = input_with_bad_plate();

    // Act
    let result = podium_poster(&input, &path);

    // Assert
    assert!(matches!(result, Err(PdfError::Layout(_))));
}

#[test]
fn test_podium_poster_to_missing_directory_returns_io_error() {
    // Arrange
    let dir = TempDir::new().expect("the fixture directory");
    let path = dir.path().join("missing").join("poster.pdf");
    let input = fixtures::golden_input();

    // Act
    let result = podium_poster(&input, &path);

    // Assert
    assert!(matches!(result, Err(PdfError::Io(_))));
}

// -------------------------------------------------------------- the poster body

#[test]
fn test_podium_poster_mixed_field_draws_teams_before_solo_riders() {
    // Arrange
    let (payload, _plan) = fixtures::golden_payload_and_plan().expect("the golden payload builds");

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(has_text(&pages, "Teams"));
    assert!(has_text(&pages, "Solo riders"));
    assert!(index_of(&pages, "Teams") < index_of(&pages, "Solo riders"));
}

#[test]
fn test_podium_poster_solo_field_lists_the_top_five_and_no_team_section() {
    // Arrange
    let input = fixtures::solo_input();
    let payload = build_payload(&input.export).expect("the solo payload builds");

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(!has_text(&pages, "Teams"));
    assert!(has_name(&pages, "Rider Five"));
    assert!(!has_name(&pages, "Rider Six"));
}

// ------------------------------------------------------- determinism and golden

/// The frozen, Rust-owned golden poster.
const GOLDEN_PODIUM: &[u8] = include_bytes!("../../fixtures/golden-podium.pdf");

#[test]
fn test_podium_poster_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let first_path = dir.path().join("first.pdf");
    let second_path = dir.path().join("second.pdf");
    let input = fixtures::golden_input();

    // Act
    podium_poster(&input, &first_path)?;
    podium_poster(&input, &second_path)?;
    let first = std::fs::read(&first_path)?;
    let second = std::fs::read(&second_path)?;

    // Assert
    assert_eq!(first, second);
    Ok(())
}

#[test]
fn test_podium_poster_matches_the_committed_golden() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("poster.pdf");
    let input = fixtures::golden_input();

    // Act
    podium_poster(&input, &path)?;
    let bytes = std::fs::read(&path)?;

    // Assert
    assert_eq!(bytes, GOLDEN_PODIUM);
    Ok(())
}
