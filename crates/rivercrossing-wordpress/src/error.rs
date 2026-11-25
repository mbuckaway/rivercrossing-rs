// SPDX-License-Identifier: GPL-3.0-only

//! The error type that every `WordPress` call returns.
//!
//! The messages reproduce the Python oracle's `wordpress.py` text verbatim, so
//! a caller's log matches what the app has always shown.

/// A `WordPress` request failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WordPressError {
    /// The URL was refused before any credential header was built.
    #[error("refusing to send WordPress credentials to a non-https URL: {url}")]
    InsecureUrl {
        /// The URL that was refused.
        url: String,
    },
    /// `WordPress` refused the credentials (HTTP 401/403).
    ///
    /// `detail` is the rendered suffix `": {code}: {message}"`, empty when the
    /// error body carried no `code`/`message` strings.
    #[error("WordPress rejected the credentials for {url} (HTTP {status}){detail}")]
    Auth {
        /// The URL that was refused.
        url: String,
        /// The HTTP status that `WordPress` answered with.
        status: u16,
        /// The rendered `": {code}: {message}"` suffix, or empty.
        detail: String,
    },
    /// `WordPress` answered with its own error document.
    #[error("WordPress error {code} (HTTP {status}): {message}")]
    Response {
        /// The `WordPress` error code.
        code: String,
        /// The `WordPress` error message.
        message: String,
        /// The HTTP status that `WordPress` answered with.
        status: u16,
    },
    /// The site is not answering the REST API as documented.
    #[error("{message}")]
    Unavailable {
        /// The operator-facing reason.
        message: String,
    },
}

impl WordPressError {
    /// Return the error for a URL that is not `https`.
    pub(crate) fn insecure(url: &str) -> Self {
        Self::InsecureUrl {
            url: url.to_owned(),
        }
    }

    /// Return the error for a refused credential set.
    ///
    /// `detail` holds the error body's `code` and `message` when both are
    /// strings; any other shape leaves the suffix empty.
    pub(crate) fn refused(url: &str, status: u16, detail: Option<(String, String)>) -> Self {
        let detail = detail.map_or_else(String::new, |(code, message)| {
            format!(": {code}: {message}")
        });
        Self::Auth {
            url: url.to_owned(),
            status,
            detail,
        }
    }

    /// Return the error for a `WordPress` error document.
    pub(crate) fn response(code: String, message: String, status: u16) -> Self {
        Self::Response {
            code,
            message,
            status,
        }
    }

    /// Return the error for a site that is not answering as documented.
    pub(crate) fn unavailable(message: impl Into<String>) -> Self {
        Self::Unavailable {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests;
