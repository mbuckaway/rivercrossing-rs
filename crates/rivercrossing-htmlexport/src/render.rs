// SPDX-License-Identifier: GPL-3.0-only

//! The results page renderer.
//!
//! [`render`] maps an [`ExportInput`] to the shared payload and renders the
//! page; [`render_page`] renders an already-built [`RacePayload`], recomputing
//! the per-kind laps boards from the placed standings when it is given them and
//! otherwise keeping the payload's own plan.

use rivercrossing_standings::Placed;

use crate::env::{environment, page_context};
use crate::error::ExportError;
use crate::payload::build_payload;
use crate::sections::{sections, sections_from_payload};
use crate::types::{ExportInput, RacePayload};

/// The page template that the environment loads.
const TEMPLATE: &str = "base.html.j2";

/// Render the results page for *input*.
///
/// # Errors
///
/// Return the payload error when a placed entry cannot be recorded, and
/// [`ExportError::Template`] when the page template cannot render.
pub fn render(input: &ExportInput) -> Result<String, ExportError> {
    let payload = build_payload(input)?;
    render_page(&payload, Some(&input.placed), input.logo_src.as_deref())
}

/// Render the results page for *payload*.
///
/// The laps boards are recomputed from *placed* when it is given and read from
/// the payload's own plan otherwise. An absent logo falls back to the
/// transparent PNG.
pub(crate) fn render_page(
    payload: &RacePayload,
    placed: Option<&[Placed]>,
    logo_src: Option<&str>,
) -> Result<String, ExportError> {
    let plan = placed.map_or_else(
        || sections_from_payload(payload),
        |placed| sections(payload, placed),
    );
    let context = page_context(payload, &plan, logo_src);
    environment()
        .get_template(TEMPLATE)
        .and_then(|template| template.render(&context))
        .map_err(|error| ExportError::Template(error.to_string()))
}

#[cfg(test)]
mod tests;
