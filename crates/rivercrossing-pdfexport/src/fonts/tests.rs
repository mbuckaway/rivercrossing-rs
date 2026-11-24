// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the embedded font faces and their advance metrics.

use super::{FontFace, glyphs_available, text_width, units_per_em};

#[test]
fn test_fonts_body_face_reports_its_design_units() {
    // Arrange / Act
    let upem = units_per_em(FontFace::Body);

    // Assert
    assert!((upem - 1000.0).abs() < f32::EPSILON, "got {upem}");
}

#[test]
fn test_fonts_heading_face_reports_its_design_units() {
    // Arrange / Act
    let upem = units_per_em(FontFace::Heading);

    // Assert
    assert!((upem - 1000.0).abs() < f32::EPSILON, "got {upem}");
}

#[test]
fn test_fonts_glyph_face_reports_its_design_units() {
    // Arrange / Act
    let upem = units_per_em(FontFace::Glyph);

    // Assert
    assert!((upem - 2048.0).abs() < f32::EPSILON, "got {upem}");
}

#[test]
fn test_fonts_text_width_is_positive_for_a_letter() {
    // Arrange / Act
    let width = text_width(FontFace::Body, "A", 10.0);

    // Assert
    assert!(width > 0.0, "got {width}");
}

#[test]
fn test_fonts_text_width_grows_with_a_second_letter() {
    // Arrange / Act
    let one = text_width(FontFace::Body, "A", 10.0);
    let two = text_width(FontFace::Body, "AA", 10.0);

    // Assert
    assert!(two > one, "one {one}, two {two}");
}

#[test]
fn test_fonts_text_width_of_empty_text_is_zero() {
    // Arrange / Act
    let width = text_width(FontFace::Glyph, "", 9.0);

    // Assert
    assert!((width - 0.0).abs() < f32::EPSILON, "got {width}");
}

#[test]
fn test_fonts_glyph_face_carries_every_suit_and_the_star() {
    // Arrange / Act
    let available = glyphs_available(FontFace::Glyph, "♠♥♦♣★");

    // Assert
    assert!(available);
}

#[test]
fn test_fonts_body_face_carries_the_punctuation_the_layout_draws() {
    // Arrange / Act
    let available = glyphs_available(FontFace::Body, "·—×");

    // Assert
    assert!(available);
}

#[test]
fn test_fonts_heading_face_carries_the_punctuation_the_layout_draws() {
    // Arrange / Act
    let available = glyphs_available(FontFace::Heading, "·—×");

    // Assert
    assert!(available);
}

#[test]
fn test_fonts_body_face_lacks_the_spade_glyph() {
    // Arrange / Act
    let available = glyphs_available(FontFace::Body, "♠");

    // Assert
    assert!(!available);
}
