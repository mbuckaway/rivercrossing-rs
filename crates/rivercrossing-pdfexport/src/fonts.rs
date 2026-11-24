// SPDX-License-Identifier: GPL-3.0-only

//! The three embedded font faces and the advance metrics the layout needs.
//!
//! `Barlow` sets body text, `Barlow Condensed SemiBold` sets headings and bold
//! table cells, and `DejaVu Sans` carries the suit glyphs (`♠♥♦♣★`) that no
//! Barlow face has. `skrifa` measures each face's advance widths so the card
//! boxes and the paragraph wrapping use real glyph metrics rather than a
//! character count.

use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

/// One embedded font face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFace {
    /// Barlow Regular: body text.
    Body,
    /// Barlow Condensed `SemiBold`: headings and bold cells.
    Heading,
    /// `DejaVu Sans`: the suit and star glyphs.
    Glyph,
}

const BODY_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fonts/Barlow-Regular.ttf"
));
const HEADING_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fonts/BarlowCondensed-SemiBold.ttf"
));
const GLYPH_BYTES: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/DejaVuSans.ttf"));

impl FontFace {
    /// Return this face's embedded TrueType bytes.
    const fn bytes(self) -> &'static [u8] {
        match self {
            Self::Body => BODY_BYTES,
            Self::Heading => HEADING_BYTES,
            Self::Glyph => GLYPH_BYTES,
        }
    }
}

/// Read one embedded face, or none when the bytes do not parse.
///
/// The bytes are compile-time assets proven by this module's tests, so the
/// `None` arm only keeps the metric functions infallible.
fn font(face: FontFace) -> Option<FontRef<'static>> {
    FontRef::new(face.bytes()).ok()
}

/// Return the face's design units per em.
#[must_use]
pub fn units_per_em(face: FontFace) -> f32 {
    font(face).map_or(0.0, |font| {
        f32::from(
            font.metrics(Size::unscaled(), LocationRef::default())
                .units_per_em,
        )
    })
}

/// Return *text*'s advance width in points at *`size_pt`*.
#[must_use]
pub fn text_width(face: FontFace, text: &str, size_pt: f32) -> f32 {
    font(face).map_or(0.0, |font| {
        let metrics = font.glyph_metrics(Size::new(size_pt), LocationRef::default());
        let charmap = font.charmap();
        text.chars()
            .filter_map(|ch| charmap.map(ch))
            .filter_map(|glyph| metrics.advance_width(glyph))
            .sum()
    })
}

/// Return whether *face* carries a glyph for every character in *text*.
#[must_use]
pub fn glyphs_available(face: FontFace, text: &str) -> bool {
    font(face).is_some_and(|font| {
        let charmap = font.charmap();
        text.chars().all(|ch| charmap.map(ch).is_some())
    })
}

#[cfg(test)]
mod tests;
