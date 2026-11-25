// SPDX-License-Identifier: GPL-3.0-only

//! REST-index discovery (`wordpress.py:195-212`).
//!
//! [`discover_with`] carries the whole behaviour. The public `discover`
//! wrapper lands in phase 4 beside the production transport, so it is not
//! declared yet.

// Only the unit tests drive discovery until the public `discover` wrapper
// lands in phase 4.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the public discover wrapper, which lands in phase 4, calls discover_with"
    )
)]

use serde_json::Value;

use crate::auth::BasicAuth;
use crate::error::WordPressError;
use crate::http::{Transport, request_json_with, rest_url};
use crate::json;

/// What a site's REST index advertises about itself.
///
/// The struct stays crate-private until the public `discover` wrapper lands in
/// phase 4; the fields are public then.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SiteInfo {
    /// The REST namespaces that the site lists, in the order it lists them.
    pub(crate) namespaces: Vec<String>,
    /// True when the site advertises Application Password authentication.
    pub(crate) application_passwords: bool,
}

/// Return what the site at `base_url` advertises through its REST index.
///
/// `namespaces` keeps only the string items that the index lists, and
/// `application_passwords` is true only when `authentication` is an object
/// that holds the `application-passwords` key.
///
/// # Errors
///
/// Return [`WordPressError::InsecureUrl`] for a non-https base URL, and
/// [`WordPressError::Unavailable`] when the site does not answer with a JSON
/// object. A refused credential set returns [`WordPressError::Auth`], and a
/// `WordPress` error document returns [`WordPressError::Response`].
pub(crate) fn discover_with(
    transport: &dyn Transport,
    base_url: &str,
    auth: &BasicAuth,
) -> Result<SiteInfo, WordPressError> {
    let url = rest_url(base_url, "wp-json/", &[]);
    let index = json::as_object(request_json_with(transport, &url, auth, None)?, &url)?;
    let application_passwords = index
        .get("authentication")
        .and_then(Value::as_object)
        .is_some_and(|mapping| mapping.contains_key("application-passwords"));
    Ok(SiteInfo {
        namespaces: json::string_items(index.get("namespaces")),
        application_passwords,
    })
}

#[cfg(test)]
mod tests;
