// SPDX-License-Identifier: GPL-3.0-only

//! The craft-pdf writer: the thin adapter that turns the page model into bytes.
//!
//! The engine's user space already has its origin at the top-left corner with
//! `y` growing downward, so an [`Op`]'s coordinates map onto the surface
//! directly. [`Op::Text`] carries the top of its cell, and the writer moves the
//! baseline one font size down. The caller supplies the timestamp, so the same
//! input yields the same bytes on every host.

use craft_pdf::write::backend::geom::{Path, PathBuilder, Point, Rect, Size, Transform};
use craft_pdf::write::backend::image::Image;
use craft_pdf::write::backend::metadata::DateTime;
use craft_pdf::write::backend::num::NormalizedF32;
use craft_pdf::write::backend::page::PageSettings;
use craft_pdf::write::backend::paint::{Fill, FillRule, Paint, Stroke};
use craft_pdf::write::backend::surface::Surface;
use craft_pdf::write::backend::text::{Font, TextDirection};
use craft_pdf::write::backend::{Document, SerializeSettings, color::rgb};
use craft_pdf::write::{Metadata, Writer, font};
use time::OffsetDateTime;

use crate::error::PdfError;
use crate::fonts::FontFace;
use crate::geometry::Color;
use crate::layout::{Op, Page};

/// The [`Op::Image`] height that asks for the PNG's own aspect ratio.
const INTRINSIC_ASPECT: f32 = 0.0;

/// The three embedded faces, loaded once per document.
struct Faces {
    body: Font,
    heading: Font,
    glyph: Font,
}

impl Faces {
    /// Load the body, heading and glyph faces.
    fn load() -> Result<Self, PdfError> {
        Ok(Self {
            body: load(FontFace::Body)?,
            heading: load(FontFace::Heading)?,
            glyph: load(FontFace::Glyph)?,
        })
    }

    /// Return the engine font for *face*.
    fn get(&self, face: FontFace) -> Font {
        match face {
            FontFace::Body => self.body.clone(),
            FontFace::Heading => self.heading.clone(),
            FontFace::Glyph => self.glyph.clone(),
        }
    }
}

/// Load *face* for embedding.
fn load(face: FontFace) -> Result<Font, PdfError> {
    font(face.bytes(), 0, None).map_err(|error| PdfError::Layout(error.to_string()))
}

/// Write *pages* as a PDF titled *title* and created at *`created_at`*.
///
/// # Errors
///
/// Return [`PdfError::Layout`] when a page or an operation carries geometry the
/// engine refuses, [`PdfError::Logo`] when an [`Op::Image`] is not a PNG, and
/// [`PdfError::Write`] when serialization fails.
pub fn write(pages: &[Page], title: &str, created_at: OffsetDateTime) -> Result<Vec<u8>, PdfError> {
    let faces = Faces::load()?;
    let mut writer = Writer::new(settings(), metadata(title, created_at));
    {
        let document = writer.document_mut();
        for page in pages {
            draw_page(document, page, &faces)?;
        }
    }
    writer
        .finish()
        .map_err(|error| PdfError::Write(error.to_string()))
}

/// Return the serialization settings: compressed, untagged and without XMP.
fn settings() -> SerializeSettings {
    SerializeSettings {
        pretty: false,
        compress_content_streams: true,
        xmp_metadata: false,
        enable_tagging: false,
        ..SerializeSettings::default()
    }
}

/// Return the document information: a title and the caller's timestamp.
fn metadata(title: &str, created_at: OffsetDateTime) -> Metadata {
    Metadata {
        title: Some(title.to_owned()),
        creator: String::new(),
        producer: String::new(),
        created: Some(to_krilla_date(created_at)),
        ..Metadata::default()
    }
}

/// Convert *dt* to the engine's date, keeping its wall clock and offset.
fn to_krilla_date(dt: OffsetDateTime) -> DateTime {
    let offset = dt.offset();
    DateTime::new(u16::try_from(dt.year()).unwrap_or(0))
        .month(u8::from(dt.month()))
        .day(dt.day())
        .hour(dt.hour())
        .minute(dt.minute())
        .second(dt.second())
        .utc_offset_hour(offset.whole_hours())
        .utc_offset_minute(offset.minutes_past_hour().unsigned_abs())
}

/// Draw one page's operations.
fn draw_page(document: &mut Document, page: &Page, faces: &Faces) -> Result<(), PdfError> {
    let page_size = PageSettings::from_wh(page.width_pt, page.height_pt).ok_or_else(|| {
        PdfError::Layout(format!(
            "a page of {} by {} points is not drawable",
            page.width_pt, page.height_pt
        ))
    })?;
    let mut pdf_page = document.start_page_with(page_size);
    let mut surface = pdf_page.surface();
    for op in &page.ops {
        draw_op(&mut surface, op, faces)?;
    }
    surface.finish();
    pdf_page.finish();
    Ok(())
}

/// Draw one operation.
///
/// The surface keeps its fill and stroke across calls, so each operation sets
/// the paint it uses and clears the other; otherwise a rectangle after a rule
/// would be stroked too, and a text run would be outlined.
#[expect(
    clippy::too_many_lines,
    reason = "one match over the four-operation page model; splitting it would only plumb the same fields twice"
)]
fn draw_op(surface: &mut Surface<'_>, op: &Op, faces: &Faces) -> Result<(), PdfError> {
    match op {
        Op::Text {
            x,
            y,
            font,
            size,
            color,
            text,
        } => {
            surface.set_stroke(None);
            surface.set_fill(Some(fill(*color)));
            surface.draw_text(
                Point::from_xy(*x, y + size),
                faces.get(*font),
                *size,
                text,
                false,
                TextDirection::Auto,
            );
            Ok(())
        }
        Op::FilledRect {
            x,
            y,
            w,
            h,
            fill: color,
        } => {
            surface.set_stroke(None);
            surface.set_fill(Some(fill(*color)));
            let rect = Rect::from_xywh(*x, *y, *w, *h)
                .ok_or_else(|| PdfError::Layout("the rectangle is not drawable".to_owned()))?;
            let mut builder = PathBuilder::new();
            builder.push_rect(rect);
            surface.draw_path(&path_from(builder)?);
            Ok(())
        }
        Op::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            width_pt,
        } => {
            surface.set_fill(None);
            surface.set_stroke(Some(Stroke {
                paint: paint(*color),
                width: *width_pt,
                ..Stroke::default()
            }));
            let mut builder = PathBuilder::new();
            builder.move_to(*x1, *y1);
            builder.line_to(*x2, *y2);
            surface.draw_path(&path_from(builder)?);
            Ok(())
        }
        Op::Image { x, y, w, h, png } => draw_image(surface, *x, *y, *w, *h, png),
    }
}

/// Draw one PNG image at *x* and *y*, keeping the PNG's aspect when *h* is the
/// intrinsic-aspect sentinel.
#[expect(
    clippy::too_many_arguments,
    reason = "the parameters are one image operation's geometry, already named by the page model"
)]
fn draw_image(
    surface: &mut Surface<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    png: &[u8],
) -> Result<(), PdfError> {
    let image = Image::from_png(png.to_vec().into(), false).map_err(PdfError::Logo)?;
    let (pixel_w, pixel_h) = image.size();
    let height = if h == INTRINSIC_ASPECT {
        aspect_height(w, pixel_w, pixel_h)
    } else {
        h
    };
    let size = Size::from_wh(w, height)
        .ok_or_else(|| PdfError::Layout("the image is not drawable".to_owned()))?;
    surface.push_transform(&Transform::from_translate(x, y));
    surface.draw_image(image, size);
    surface.pop();
    Ok(())
}

/// Return the height that keeps the PNG's own aspect ratio at *`width_pt`*.
#[expect(
    clippy::cast_precision_loss,
    reason = "PNG pixel counts are far below f32's exact integer range"
)]
fn aspect_height(width_pt: f32, pixel_w: u32, pixel_h: u32) -> f32 {
    width_pt * (pixel_h as f32 / pixel_w as f32)
}

/// Return the path *builder* describes.
fn path_from(builder: PathBuilder) -> Result<Path, PdfError> {
    builder
        .finish()
        .ok_or_else(|| PdfError::Layout("the path is not finite".to_owned()))
}

/// Return *color*'s paint.
fn paint(color: Color) -> Paint {
    rgb::Color::new(color.r, color.g, color.b).into()
}

/// Return a fill in *color*.
fn fill(color: Color) -> Fill {
    Fill {
        paint: paint(color),
        opacity: NormalizedF32::ONE,
        rule: FillRule::default(),
    }
}

#[cfg(test)]
mod tests;
