// SPDX-License-Identifier: GPL-3.0-only

//! The `WordPress` Application Password, encoded for the `Authorization` header.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

/// A `WordPress` login name and Application Password.
///
/// wp-admin displays an Application Password in space-separated chunks, but
/// `WordPress` compares the unspaced value, so [`BasicAuth::header_value`]
/// removes every ASCII space from the password before encoding it.
#[derive(Clone, PartialEq, Eq)]
pub struct BasicAuth {
    username: String,
    password: String,
}

impl BasicAuth {
    /// Return the credentials for one `WordPress` user.
    ///
    /// The password is kept as given; [`BasicAuth::header_value`] removes its
    /// display spaces when it builds the header.
    #[must_use]
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Return `Basic base64("{username}:{password}")`.
    ///
    /// Every ASCII space is removed from the password first, so a password
    /// copied in its wp-admin chunks yields the header `WordPress` expects.
    #[must_use]
    pub fn header_value(&self) -> String {
        let unspaced = self.password.replace(' ', "");
        format!(
            "Basic {}",
            STANDARD.encode(format!("{}:{unspaced}", self.username))
        )
    }
}

/// Render the credentials with the password redacted.
///
/// A credential in a log line is a security defect, so the manual `Debug`
/// never prints the password.
impl std::fmt::Debug for BasicAuth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BasicAuth")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests;
