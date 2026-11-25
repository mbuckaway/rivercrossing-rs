// SPDX-License-Identifier: GPL-3.0-only

//! The app's only `WordPress` surface (R-86): a blocking, single-site REST
//! client.
//!
//! `BasicAuth` carries a `WordPress` user's login name and Application
//! Password. `discover` reads `{base}/wp-json/` and reports what the site
//! advertises; `find_page_by_slug` asks `wp/v2/pages` for the page at a slug
//! and returns its id, or `None`; `publish_page` creates the page when the
//! `PageDraft` carries no `page_id` and updates it otherwise.
//!
//! Every request is HTTPS-only: a non-https base URL is refused before the
//! `Authorization` header is built, so a plain-`http` site never sees the
//! Application Password. Nothing here touches the UI or the store — the
//! caller renders the page HTML and persists the credentials.
//!
//! The HTTP boundary is the crate-private `http::Transport` seam; the
//! production implementation is `transport::UreqTransport`, and the unit
//! tests substitute a recording mock.

#![forbid(unsafe_code)]

mod auth;
mod discover;
mod error;
mod http;
mod json;

#[cfg(test)]
pub(crate) mod mock;

pub use auth::BasicAuth;
pub use error::WordPressError;
