// SPDX-License-Identifier: GPL-3.0-only

//! The Publish-to-`WordPress` dialog's decisions, with no I/O.
//!
//! The module supplies the values the dialog opens with, the label-to-value
//! maps its status and publish-kind choices render, and the rules the
//! operator's input has to pass before a publish is attempted. The HTTP work
//! belongs to `rivercrossing-wordpress`; nothing here talks to a site.
//!
//! The rules follow the Python oracle's `ui/presenters/publish_wordpress.py`.
//! The slug algorithm is the same transform that names every export file, so
//! one ride has one slug across the exported file name and the published page
//! URL.

#[cfg(test)]
mod tests;

/// The status choice's items, label then value, in render order.
///
/// `WordPress`'s `future` status is deliberately absent: it schedules a post
/// against a date this dialog never collects.
pub(crate) const STATUS_CHOICES: [(&str, &str); 4] = [
    ("Draft", "draft"),
    ("Publish", "publish"),
    ("Pending", "pending"),
    ("Private", "private"),
];

/// The publish-kind choice's items, label then value, in render order.
///
/// The published page either carries every entry (`full`) or just the podium
/// (`podium`); the full page is first, so it is the choice's opening item.
pub(crate) const PUBLISH_KIND_CHOICES: [(&str, &str); 2] =
    [("Full results", "full"), ("Podium results", "podium")];

/// The status an unrecognized label resolves to: draft, so a blank or
/// unexpected control value never publishes by accident.
const DEFAULT_STATUS: &str = "draft";

/// The kind an unrecognized label resolves to: the full page, so a blank or
/// unexpected control value never narrows a page to the podium by accident.
const DEFAULT_KIND: &str = "full";

/// The podium kind's value.
const PODIUM_KIND: &str = "podium";

/// A slug is mandatory, so a nameless ride falls back to this word, the same
/// one the export filenames use.
const FALLBACK_SLUG: &str = "results";

/// The scheme an operator's site URL must carry.
const HTTPS_PREFIX: &str = "https://";

/// The refusal for a site URL that is not HTTPS.
const HTTPS_REQUIRED: &str =
    "site URL must start with https:// (WordPress credentials are only sent over HTTPS)";

/// The publish dialog's field values at one instant.
///
/// The view builds it when the operator asks to publish and the publish path
/// reads it, so the dialog cannot change out from under a publish already in
/// flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublishForm {
    /// The site's base URL, such as `https://example.test`.
    pub(crate) base_url: String,
    /// The site's own `WordPress` login name.
    pub(crate) username: String,
    /// The Application Password generated for that user.
    pub(crate) password: String,
    /// The page title.
    pub(crate) title: String,
    /// The page's URL slug.
    pub(crate) slug: String,
    /// The parent page's id or slug as typed; empty is top level.
    pub(crate) parent: String,
    /// The `WordPress` status to ask for, one of [`STATUS_CHOICES`]' values.
    pub(crate) status: String,
    /// The page the results carry, one of [`PUBLISH_KIND_CHOICES`]' values.
    pub(crate) kind: String,
}

impl Default for PublishForm {
    /// Return the dialog's empty form, opening on the full-results page.
    fn default() -> Self {
        Self {
            base_url: String::new(),
            username: String::new(),
            password: String::new(),
            title: String::new(),
            slug: String::new(),
            parent: String::new(),
            status: DEFAULT_STATUS.to_owned(),
            kind: DEFAULT_KIND.to_owned(),
        }
    }
}

impl PublishForm {
    /// Return whether every field passes [`validate`].
    #[must_use]
    pub(crate) fn is_valid(&self) -> bool {
        self.errors().is_empty()
    }

    /// Return the form's validation messages, empty when the form is valid.
    #[must_use]
    pub(crate) fn errors(&self) -> Vec<String> {
        validate(self)
    }
}

/// Return the `WordPress` status that a [`STATUS_CHOICES`] label carries.
///
/// Any other label, including a blank one, resolves to `draft`.
#[must_use]
pub(crate) fn status_value(label: &str) -> &'static str {
    choice_value(&STATUS_CHOICES, label).unwrap_or(DEFAULT_STATUS)
}

/// Return the publish kind that a [`PUBLISH_KIND_CHOICES`] label carries.
///
/// Any other label, including a blank one, resolves to `full`.
#[must_use]
pub(crate) fn kind_value(label: &str) -> &'static str {
    choice_value(&PUBLISH_KIND_CHOICES, label).unwrap_or(DEFAULT_KIND)
}

/// Return the value that `choices` holds for `label`, when one matches.
fn choice_value(choices: &[(&'static str, &'static str)], label: &str) -> Option<&'static str> {
    choices
        .iter()
        .find(|(rendered, _value)| *rendered == label)
        .map(|(_rendered, value)| *value)
}

/// Return *name* as a URL slug.
///
/// The text is lowercased, every non-alphanumeric character becomes `-`, runs
/// of `-` collapse, and the edges are stripped. A name with no alphanumeric
/// character at all becomes [`FALLBACK_SLUG`]. Non-ASCII letters and digits
/// are kept, because `WordPress` accepts them too.
#[must_use]
pub(crate) fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for character in name.to_lowercase().chars() {
        let mapped = if character.is_alphanumeric() {
            character
        } else {
            '-'
        };
        if mapped == '-' && (slug.is_empty() || slug.ends_with('-')) {
            continue;
        }
        slug.push(mapped);
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        return FALLBACK_SLUG.to_owned();
    }
    slug
}

/// Return the publish kind that `kind` names: a choice value or a label.
///
/// Anything else opens the full page, never the podium.
#[must_use]
pub(crate) fn resolved_kind(kind: &str) -> &'static str {
    if let Some((_label, value)) = PUBLISH_KIND_CHOICES
        .iter()
        .find(|(_label, value)| *value == kind)
    {
        return value;
    }
    kind_value(kind)
}

/// Return the page title the dialog opens with.
///
/// The title is `"<name> — Full Results"`, or `"<name> — Podium Results"` for
/// the podium kind.
#[must_use]
pub(crate) fn default_title(name: &str, kind: &str) -> String {
    let suffix = if resolved_kind(kind) == PODIUM_KIND {
        "Podium Results"
    } else {
        "Full Results"
    };
    format!("{name} — {suffix}")
}

/// Return the page slug the dialog opens with.
///
/// The slug is [`slugify`] of `"<name> full results"`, or of `"<name> podium
/// results"` for the podium kind, so a nameless ride still yields a non-empty
/// slug.
#[must_use]
pub(crate) fn default_slug(name: &str, kind: &str) -> String {
    let tail = if resolved_kind(kind) == PODIUM_KIND {
        "podium results"
    } else {
        "full results"
    };
    slugify(&format!("{name} {tail}"))
}

/// Return why the dialog's fields cannot be published.
///
/// Messages are ordered top of form first, and each names the field it is
/// about. No site is contacted. The parent page and the status choice are never
/// errors: a blank parent means top level, and the status is one of the
/// choice's values.
#[must_use]
pub(crate) fn validate(form: &PublishForm) -> Vec<String> {
    let mut errors = Vec::new();
    if form.base_url.is_empty() {
        errors.push("site URL is required".to_owned());
    } else if !form.base_url.starts_with(HTTPS_PREFIX) {
        errors.push(HTTPS_REQUIRED.to_owned());
    }
    push_required(&mut errors, &form.username, "username is required");
    push_required(
        &mut errors,
        &form.password,
        "application password is required",
    );
    push_required(&mut errors, &form.title, "page title is required");
    push_required(&mut errors, &form.slug, "slug is required");
    errors
}

/// Push `message` when `value` is empty.
fn push_required(errors: &mut Vec<String>, value: &str, message: &str) {
    if value.is_empty() {
        errors.push(message.to_owned());
    }
}
