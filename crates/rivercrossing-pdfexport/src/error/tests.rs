// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the PDF exporter's error type.

use pretty_assertions::assert_eq;

use super::PdfError;

#[test]
fn test_pdf_error_logo_display_names_the_logo_problem() {
    // Arrange
    let error = PdfError::Logo("not a PNG".to_owned());

    // Act
    let message = error.to_string();

    // Assert
    assert_eq!(message, "logo: not a PNG");
}

#[test]
fn test_pdf_error_layout_display_names_the_layout_problem() {
    // Arrange
    let error = PdfError::Layout("page is too narrow".to_owned());

    // Act
    let message = error.to_string();

    // Assert
    assert_eq!(message, "layout: page is too narrow");
}

#[test]
fn test_pdf_error_write_display_names_the_write_problem() {
    // Arrange
    let error = PdfError::Write("disk is full".to_owned());

    // Act
    let message = error.to_string();

    // Assert
    assert_eq!(message, "write: disk is full");
}

#[test]
fn test_pdf_error_from_io_wraps_the_message() {
    // Arrange
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");

    // Act
    let error = PdfError::from(io);

    // Assert
    assert!(matches!(error, PdfError::Io(_)));
    assert_eq!(error.to_string(), "io: missing");
}

#[test]
fn test_pdf_error_io_source_is_the_wrapped_error() {
    // Arrange
    use std::error::Error as _;

    let error = PdfError::from(std::io::Error::other("boom"));

    // Act
    let source = error.source().map(ToString::to_string);

    // Assert
    assert_eq!(source, Some("boom".to_owned()));
}
