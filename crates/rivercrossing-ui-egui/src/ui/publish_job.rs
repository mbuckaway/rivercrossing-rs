// SPDX-License-Identifier: GPL-3.0-only

//! The `WordPress` publish job: render the fragment, find the page, create or update it.
//!
//! The job runs on its own thread (R-86), so the blocking HTTP calls never touch the UI thread.
//! Cancel is honoured at every checkpoint: before the lookup, before the publish, and after the
//! publish returns. The writer seam is a trait so the orchestration is unit-tested without a site.

use rivercrossing_wordpress::{
    BasicAuth, PageDraft, PublishedPage, find_page_by_slug, publish_page,
};

use crate::ui::presenters::export_input;
use crate::ui::state::exports::PublishStart;

#[cfg(test)]
mod tests;

/// The two network calls the publish makes.
#[cfg_attr(test, mockall::automock)]
#[expect(
    clippy::too_many_arguments,
    reason = "the seam mirrors the crate's own (base, auth, slug|draft) parameter list"
)]
pub(crate) trait PageWriter {
    /// Return the id of the page at `slug`, or `None` when the site has none.
    ///
    /// # Errors
    ///
    /// Return the crate's own sentence when the lookup fails.
    fn find(&self, base_url: &str, auth: &BasicAuth, slug: &str) -> Result<Option<i64>, String>;

    /// Create or update the page that `draft` names.
    ///
    /// # Errors
    ///
    /// Return the crate's own sentence when the publish fails.
    #[expect(
        clippy::elidable_lifetime_names,
        reason = "the trait object's PageDraft borrow names its lifetime for the mockall double"
    )]
    fn publish<'a>(
        &self,
        base_url: &str,
        auth: &BasicAuth,
        draft: &PageDraft<'a>,
    ) -> Result<PublishedPage, String>;
}

/// The real writer: the `rivercrossing-wordpress` crate over its HTTPS transport.
pub(crate) struct WordPressWriter;

impl PageWriter for WordPressWriter {
    fn find(&self, base_url: &str, auth: &BasicAuth, slug: &str) -> Result<Option<i64>, String> {
        find_page_by_slug(base_url, auth, slug).map_err(|error| error.to_string())
    }

    #[expect(
        clippy::elidable_lifetime_names,
        reason = "the trait object's PageDraft borrow names its lifetime for the mockall double"
    )]
    fn publish<'a>(
        &self,
        base_url: &str,
        auth: &BasicAuth,
        draft: &PageDraft<'a>,
    ) -> Result<PublishedPage, String> {
        publish_page(base_url, auth, draft).map_err(|error| error.to_string())
    }
}

/// How one publish attempt ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PublishOutcome {
    /// The site created or updated the page.
    Published(PublishedPage),
    /// The operator cancelled at a checkpoint; nothing further happened.
    Cancelled,
}

/// Run one publish attempt, checking the operator's cancel request at each checkpoint.
///
/// # Errors
///
/// Return the render refusal or the site's own sentence.
pub(crate) fn run_publish(
    writer: &dyn PageWriter,
    start: &PublishStart,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<PublishOutcome, String> {
    let content = if start.form.kind == "podium" {
        export_input::render_wordpress_podium(&start.input)?
    } else {
        export_input::render_wordpress_full(&start.input)?
    };
    let auth = BasicAuth::new(start.form.username.clone(), start.form.password.clone());
    if is_cancelled() {
        return Ok(PublishOutcome::Cancelled);
    }
    let page_id = writer.find(&start.form.base_url, &auth, &start.form.slug)?;
    if is_cancelled() {
        return Ok(PublishOutcome::Cancelled);
    }
    let parent = resolve_parent(writer, &start.form.base_url, &auth, &start.form.parent)?;
    if is_cancelled() {
        return Ok(PublishOutcome::Cancelled);
    }
    let draft = PageDraft {
        title: &start.form.title,
        slug: &start.form.slug,
        content: &content,
        status: &start.form.status,
        parent,
        page_id,
    };
    let page = writer.publish(&start.form.base_url, &auth, &draft)?;
    Ok(PublishOutcome::Published(page))
}

/// Return the parent page id that *parent* names.
///
/// A blank parent is top level (`0`), a numeric value is the page id, and anything else is a slug
/// the site resolves. A slug the site does not hold is top level too, exactly as the Python
/// `_resolve_parent` treats it; a failed lookup is the publish's failure, never a silent `0`.
///
/// # Errors
///
/// Return the site's own sentence when the slug lookup fails.
#[expect(
    clippy::too_many_arguments,
    reason = "the resolver needs the writer, the site's base URL and credentials, and the typed parent"
)]
fn resolve_parent(
    writer: &dyn PageWriter,
    base_url: &str,
    auth: &BasicAuth,
    parent: &str,
) -> Result<i64, String> {
    let trimmed = parent.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    if let Ok(id) = trimmed.parse::<i64>() {
        return Ok(id);
    }
    Ok(writer.find(base_url, auth, trimmed)?.unwrap_or(0))
}
