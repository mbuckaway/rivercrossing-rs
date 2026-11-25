// SPDX-License-Identifier: GPL-3.0-only

//! Page lookup and publish (`wordpress.py:238-299`).
//!
//! [`find_page_by_slug_with`] asks `wp/v2/pages` for the page at a slug, and
//! [`publish_page_with`] creates the page when the draft carries no `page_id`
//! and updates it otherwise.

// Only the unit tests drive these functions until the public wrappers land in
// phase 4.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the public find_page_by_slug and publish_page wrappers, which land in phase 4, call these functions"
    )
)]
#![expect(
    clippy::too_many_arguments,
    reason = "the page seam keeps the oracle's parameter lists, and PageDraft::new mirrors the four caller-supplied fields"
)]

use serde_json::json;

use crate::auth::BasicAuth;
use crate::error::WordPressError;
use crate::http::{Transport, request_json_with, rest_url};
use crate::json::{as_array, as_object, integer_field, text_field};

/// The page a publish call created or updated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublishedPage {
    /// The page's id on the site.
    pub(crate) id: i64,
    /// The page's public URL.
    pub(crate) link: String,
    /// The page's status, as the site reports it.
    pub(crate) status: String,
    /// True when the call created the page; false when it updated one.
    pub(crate) created: bool,
}

/// The page fields a caller publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageDraft<'a> {
    /// The page title.
    pub(crate) title: &'a str,
    /// The page slug.
    pub(crate) slug: &'a str,
    /// The page body as HTML.
    pub(crate) content: &'a str,
    /// The page status to write.
    pub(crate) status: &'a str,
    /// The parent page id; 0 for a top-level page.
    pub(crate) parent: i64,
    /// The page id to update; `None` creates a page.
    pub(crate) page_id: Option<i64>,
}

impl<'a> PageDraft<'a> {
    /// Return a draft that creates a new top-level page.
    ///
    /// `parent` is 0 and `page_id` is `None`, so a publish creates the page.
    pub(crate) fn new(title: &'a str, slug: &'a str, content: &'a str, status: &'a str) -> Self {
        Self {
            title,
            slug,
            content,
            status,
            parent: 0,
            page_id: None,
        }
    }
}

/// Return the id of the first page at `slug`, or `None` when the site lists none.
///
/// The query asks for `context=edit`, so the site returns drafts as well.
///
/// # Errors
///
/// Return [`WordPressError::InsecureUrl`] for a non-https base URL, and
/// [`WordPressError::Unavailable`] when the site does not answer with a JSON
/// array of pages that carry an integer `id`. A refused credential set returns
/// [`WordPressError::Auth`], and a `WordPress` error document returns
/// [`WordPressError::Response`].
pub(crate) fn find_page_by_slug_with(
    transport: &dyn Transport,
    base_url: &str,
    auth: &BasicAuth,
    slug: &str,
) -> Result<Option<i64>, WordPressError> {
    let url = rest_url(
        base_url,
        "wp-json/wp/v2/pages",
        &[("slug", slug), ("context", "edit")],
    );
    let payload = request_json_with(transport, &url, auth, None)?;
    let pages = as_array(&payload, &url)?;
    let Some(first) = pages.first() else {
        return Ok(None);
    };
    Ok(Some(integer_field(first, "id", &url)?))
}

/// Return the page that the site created or updated for `draft`.
///
/// The body carries `title`, `slug`, `status`, `parent` and `content`, with
/// `content` as the bare HTML string. `created` reports which call the site
/// received: a create when the draft carried no `page_id`, an update
/// otherwise.
///
/// # Errors
///
/// Return [`WordPressError::InsecureUrl`] for a non-https base URL, and
/// [`WordPressError::Unavailable`] when the site does not answer with a JSON
/// object that carries an integer `id` and string `link` and `status`. A
/// refused credential set returns [`WordPressError::Auth`], and a `WordPress`
/// error document returns [`WordPressError::Response`].
pub(crate) fn publish_page_with(
    transport: &dyn Transport,
    base_url: &str,
    auth: &BasicAuth,
    draft: &PageDraft<'_>,
) -> Result<PublishedPage, WordPressError> {
    let created = draft.page_id.is_none();
    let path = draft.page_id.map_or_else(
        || "wp-json/wp/v2/pages".to_owned(),
        |page_id| format!("wp-json/wp/v2/pages/{page_id}"),
    );
    let url = rest_url(base_url, &path, &[]);
    let body = json!({
        "title": draft.title,
        "slug": draft.slug,
        "status": draft.status,
        "parent": draft.parent,
        "content": draft.content,
    });
    let payload = serde_json::to_vec(&body).map_err(|error| {
        WordPressError::unavailable(format!("WordPress page body could not be encoded: {error}"))
    })?;
    let response = request_json_with(transport, &url, auth, Some(payload))?;
    let page = as_object(&response, &url)?;
    Ok(PublishedPage {
        id: integer_field(&response, "id", &url)?,
        link: text_field(page, "link", &url)?,
        status: text_field(page, "status", &url)?,
        created,
    })
}

#[cfg(test)]
mod tests;
