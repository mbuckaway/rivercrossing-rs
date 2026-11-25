// SPDX-License-Identifier: GPL-3.0-only

//! REST-index discovery, and the `discover` entry point
//! (`wordpress.py:195-212`).

use serde_json::Value;

use crate::auth::BasicAuth;
use crate::error::WordPressError;
use crate::http::{Transport, request_json_with, rest_url};
use crate::json;
use crate::transport::UreqTransport;

/// What a site's REST index advertises about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteInfo {
    /// The REST namespaces that the site lists, in the order it lists them.
    pub namespaces: Vec<String>,
    /// True when the site advertises Application Password authentication.
    pub application_passwords: bool,
}

/// Discover what `{base_url}/wp-json/` advertises about the site.
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
pub fn discover(base_url: &str, auth: &BasicAuth) -> Result<SiteInfo, WordPressError> {
    discover_with(&UreqTransport::new(), base_url, auth)
}

/// Return what the site at `base_url` advertises through `transport`.
///
/// # Errors
///
/// Return the same errors as [`discover`].
pub(crate) fn discover_with(
    transport: &dyn Transport,
    base_url: &str,
    auth: &BasicAuth,
) -> Result<SiteInfo, WordPressError> {
    let url = rest_url(base_url, "wp-json/", &[]);
    let payload = request_json_with(transport, &url, auth, None)?;
    let index = json::as_object(&payload, &url)?;
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
