// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the results-report entry point.

use std::collections::BTreeMap;
use std::path::Path;

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;
use rivercrossing_hands::best_hand;
use rivercrossing_htmlexport::types::{ExportInput, ExportOptions, RideInfo};
use rivercrossing_ride::EntryId;
use rivercrossing_standings::{EntryResult, Placed, SnapshotKind};
use tempfile::TempDir;
use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

use super::{PdfInput, render, temp_path, write_bytes_atomic, write_bytes_atomic_with};
use crate::error::PdfError;
use crate::geometry::{A4_HEIGHT_PT, A4_WIDTH_PT, LETTER_HEIGHT_PT, LETTER_WIDTH_PT};
use crate::pdf_read;

/// The 1×1 transparent PNG (`rivercrossing_htmlexport::types::TRANSPARENT_PNG`).
const PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0xcf, 0x50,
    0x0f, 0x00, 0x03, 0x86, 0x01, 0x80, 0x5a, 0x34, 0x7d, 0x6b, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn stamp() -> OffsetDateTime {
    let date = Date::from_calendar_date(2026, Month::September, 20).expect("the fixture date");
    let clock = Time::from_hms(16, 7, 0).expect("the fixture clock");
    OffsetDateTime::new_in_offset(date, clock, UtcOffset::UTC)
}

fn ride() -> RideInfo {
    RideInfo {
        name: "Test Poker Run 2026".to_owned(),
        event_date: Date::from_calendar_date(2026, Month::June, 6).expect("the fixture date"),
        venue: "Test Venue".to_owned(),
        lap_km: 8.0,
        organizer: "Test Org".to_owned(),
        scorer: "T. Ester".to_owned(),
    }
}

fn solo_row(index: usize) -> Placed {
    let cards: Vec<Card> = ["9S", "9D", "9C", "KH", "2S"]
        .iter()
        .map(|code| Card::parse(code).expect("the fixture card code is valid"))
        .collect();
    let number = u32::try_from(index).expect("the fixture index fits");
    let plate = format!("{}", 701 + index);
    let result = EntryResult {
        entry_id: EntryId(plate.clone()),
        plate,
        name: format!("Rider {index}"),
        kind: SnapshotKind::Solo,
        laps: 60u32.saturating_sub(number),
        total_time: f64::from(60u32.saturating_sub(number)) * 1800.0 + 120.0,
        best_lap: 1800.0,
        hand: best_hand(&cards),
        cards,
        dnf: false,
        sex: Some("F".to_owned()),
        tiebreak_card: None,
    };
    Placed {
        place: Some(number + 1),
        result,
        tie_note: None,
        draw_required: false,
    }
}

fn input(rows: usize) -> PdfInput {
    PdfInput {
        export: ExportInput {
            ride: ride(),
            placed: (0..rows).map(solo_row).collect(),
            options: ExportOptions::default(),
            generated: "Generated 16:07, Sept 20 2026".to_owned(),
            logo_src: None,
            team_logos: BTreeMap::new(),
            riders: rows,
            self_test_unverified: false,
            tie_note: None,
        },
        letter: true,
        created_at: stamp(),
        logo_png: None,
    }
}

fn input_with_bad_plate() -> PdfInput {
    let mut input = input(1);
    for row in &mut input.export.placed {
        row.result.plate = "not-a-number".to_owned();
    }
    input
}

// ---------------------------------------------------------------- the report

#[test]
fn test_render_single_row_report_writes_one_page() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("report.pdf");
    let input = input(1);

    // Act
    render(&input, &path)?;
    let bytes = std::fs::read(&path)?;

    // Assert
    assert_eq!(pdf_read::page_count(&bytes), 1);
    Ok(())
}

#[test]
fn test_render_fifty_row_report_writes_at_least_two_pages() -> Result<(), Box<dyn std::error::Error>>
{
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("report.pdf");
    let input = input(50);

    // Act
    render(&input, &path)?;
    let bytes = std::fs::read(&path)?;

    // Assert
    let pages = pdf_read::page_count(&bytes);
    assert!(
        pages >= 2,
        "a 50-row report spans more than one page, got {pages}"
    );
    Ok(())
}

#[test]
fn test_render_same_input_twice_returns_identical_bytes() -> Result<(), Box<dyn std::error::Error>>
{
    // Arrange
    let dir = TempDir::new()?;
    let first_path = dir.path().join("first.pdf");
    let second_path = dir.path().join("second.pdf");
    let input = input(2);

    // Act
    render(&input, &first_path)?;
    render(&input, &second_path)?;
    let first = std::fs::read(&first_path)?;
    let second = std::fs::read(&second_path)?;

    // Assert
    assert_eq!(first, second);
    Ok(())
}

#[test]
fn test_render_letter_page_is_letter_sized() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("report.pdf");
    let mut input = input(1);
    input.letter = true;

    // Act
    render(&input, &path)?;
    let bytes = std::fs::read(&path)?;

    // Assert
    assert!(pdf_read::media_box_contains(
        &bytes,
        LETTER_WIDTH_PT,
        LETTER_HEIGHT_PT
    ));
    assert!(!pdf_read::media_box_contains(
        &bytes,
        A4_WIDTH_PT,
        A4_HEIGHT_PT
    ));
    Ok(())
}

#[test]
fn test_render_a4_page_is_a4_sized() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("report.pdf");
    let mut input = input(1);
    input.letter = false;

    // Act
    render(&input, &path)?;
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
fn test_render_without_logo_embeds_no_image() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("report.pdf");
    let input = input(1);

    // Act
    render(&input, &path)?;
    let bytes = std::fs::read(&path)?;

    // Assert
    assert!(!pdf_read::has_name(&bytes, "/Image"));
    Ok(())
}

#[test]
fn test_render_with_logo_embeds_the_image() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("report.pdf");
    let mut input = input(1);
    input.logo_png = Some(PIXEL_PNG.to_vec());

    // Act
    render(&input, &path)?;
    let bytes = std::fs::read(&path)?;

    // Assert
    assert!(pdf_read::has_name(&bytes, "/Image"));
    Ok(())
}

#[test]
fn test_render_with_non_png_logo_returns_logo_error() {
    // Arrange
    let dir = TempDir::new().expect("the fixture directory");
    let path = dir.path().join("report.pdf");
    let mut input = input(1);
    input.logo_png = Some(b"not a png".to_vec());

    // Act
    let result = render(&input, &path);

    // Assert
    assert!(matches!(result, Err(PdfError::Logo(_))));
}

#[test]
fn test_render_with_non_numeric_plate_returns_layout_error() {
    // Arrange
    let dir = TempDir::new().expect("the fixture directory");
    let path = dir.path().join("report.pdf");
    let input = input_with_bad_plate();

    // Act
    let result = render(&input, &path);

    // Assert
    assert!(matches!(result, Err(PdfError::Layout(_))));
}

#[test]
fn test_render_to_missing_directory_returns_io_error() {
    // Arrange
    let dir = TempDir::new().expect("the fixture directory");
    let path = dir.path().join("missing").join("report.pdf");
    let input = input(1);

    // Act
    let result = render(&input, &path);

    // Assert
    assert!(matches!(result, Err(PdfError::Io(_))));
}

// ------------------------------------------------------------- atomic writes

#[test]
fn test_write_bytes_atomic_replaces_the_existing_file() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("report.pdf");
    std::fs::write(&path, b"old")?;

    // Act
    write_bytes_atomic(&path, b"new")?;

    // Assert
    assert_eq!(std::fs::read(&path)?, b"new");
    assert!(!temp_path(&path)?.exists());
    Ok(())
}

#[test]
fn test_write_bytes_atomic_keeps_destination_when_rename_fails()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = TempDir::new()?;
    let path = dir.path().join("report.pdf");
    std::fs::write(&path, b"old")?;

    // Act
    let result = write_bytes_atomic_with(&path, b"new", |_, _| {
        Err(std::io::Error::other("the rename is refused"))
    });

    // Assert
    assert_eq!(
        result.map_err(|error| error.kind()),
        Err(std::io::ErrorKind::Other)
    );
    assert_eq!(std::fs::read(&path)?, b"old");
    assert_eq!(std::fs::read(temp_path(&path)?)?, b"new");
    Ok(())
}

#[test]
fn test_temp_path_of_a_root_path_returns_err() {
    // Act
    let result = temp_path(Path::new("/"));

    // Assert
    assert_eq!(
        result.map_err(|error| error.kind()),
        Err(std::io::ErrorKind::InvalidInput)
    );
}
