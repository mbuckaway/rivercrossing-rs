// SPDX-License-Identifier: GPL-3.0-only

//! The results page, poster and `WordPress` fragment renderers.
//!
//! [`render`] maps an [`ExportInput`] to the shared payload and renders the
//! page; [`render_page`] renders an already-built [`RacePayload`], recomputing
//! the per-kind laps boards from the placed standings when it is given them and
//! otherwise keeping the payload's own plan. [`render_wordpress`] renders that
//! same plan as a `WordPress` content fragment, while [`render_poster`] and
//! [`render_poster_wordpress`] render the podium poster page and its fragment.

use std::collections::BTreeMap;

use minijinja::Value;
use rivercrossing_standings::Placed;

use crate::env::{
    environment, page_context, poster_context, wordpress_page_context, wordpress_poster_context,
};
use crate::error::ExportError;
use crate::payload::build_payload;
use crate::sections::{sections, sections_from_payload};
use crate::types::{ExportInput, RacePayload};

/// The page template that the environment loads.
const TEMPLATE: &str = "base.html.j2";

/// The `WordPress` results-fragment template.
const WORDPRESS_TEMPLATE: &str = "wordpress.html.j2";

/// The podium-poster template.
const POSTER_TEMPLATE: &str = "poster.html.j2";

/// The `WordPress` podium-poster fragment template.
const POSTER_WORDPRESS_TEMPLATE: &str = "poster_wordpress.html.j2";

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
    render_template(TEMPLATE, &context)
}

/// Render *input* as a `WordPress` page-content fragment.
///
/// The fragment carries the page's own macros and per-kind plan, wrapped for a
/// page that already has a document, and inlines the scoped stylesheet.
///
/// # Errors
///
/// Return the payload error when a placed entry cannot be recorded, and
/// [`ExportError::Template`] when the fragment template cannot render.
pub fn render_wordpress(input: &ExportInput) -> Result<String, ExportError> {
    let payload = build_payload(input)?;
    let plan = sections(&payload, &input.placed);
    let context = wordpress_page_context(&payload, &plan, input.logo_src.as_deref());
    render_template(WORDPRESS_TEMPLATE, &context)
}

/// Render *input* as the self-contained podium poster page.
///
/// The poster takes the ride logo only: the payload is built without the
/// per-entry team logos, none of which a card renders.
///
/// # Errors
///
/// Return the payload error when a placed entry cannot be recorded, and
/// [`ExportError::Template`] when the poster template cannot render.
pub fn render_poster(input: &ExportInput) -> Result<String, ExportError> {
    let payload = poster_payload(input)?;
    render_poster_payload(&payload, input.logo_src.as_deref())
}

/// Render *input* as the `WordPress` podium-poster fragment.
///
/// The poster's cards in the fragment's shell, with the same logo-only payload
/// as [`render_poster`].
///
/// # Errors
///
/// Return the payload error when a placed entry cannot be recorded, and
/// [`ExportError::Template`] when the fragment template cannot render.
pub fn render_poster_wordpress(input: &ExportInput) -> Result<String, ExportError> {
    let payload = poster_payload(input)?;
    let context = wordpress_poster_context(&payload, input.logo_src.as_deref());
    render_template(POSTER_WORDPRESS_TEMPLATE, &context)
}

/// Build the payload that the poster renders from *input*.
///
/// The poster takes no per-entry logo map, so the clone drops it.
fn poster_payload(input: &ExportInput) -> Result<RacePayload, ExportError> {
    let mut poster_input = input.clone();
    poster_input.team_logos = BTreeMap::new();
    build_payload(&poster_input)
}

/// Render *payload* as the podium poster page.
///
/// The poster derives its `(heading, rows)` plan from the payload itself, so
/// it takes no section plan.
///
/// # Errors
///
/// Return [`ExportError::Template`] when the poster template cannot render.
pub(crate) fn render_poster_payload(
    payload: &RacePayload,
    logo_src: Option<&str>,
) -> Result<String, ExportError> {
    let context = poster_context(payload, logo_src);
    render_template(POSTER_TEMPLATE, &context)
}

/// Render *context* through the named vendored *template*.
fn render_template(template: &str, context: &Value) -> Result<String, ExportError> {
    environment()
        .get_template(template)
        .and_then(|template| template.render(context))
        .map_err(|error| ExportError::Template(error.to_string()))
}

#[cfg(test)]
mod tests;
