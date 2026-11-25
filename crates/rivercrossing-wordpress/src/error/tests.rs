// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the `WordPress` error type and its oracle messages.

use pretty_assertions::assert_eq;

use super::WordPressError;

#[test]
fn test_error_display_names_each_variant() {
    // Arrange
    let insecure = WordPressError::insecure("http://example.test/wp-json/");
    let refused_without_detail =
        WordPressError::refused("https://example.test/wp-json/", 403, None);
    let refused_with_detail = WordPressError::refused(
        "https://example.test/wp-json/",
        401,
        Some((
            "invalid_username".to_owned(),
            "Unknown username.".to_owned(),
        )),
    );
    let response = WordPressError::response(
        "rest_cannot_create".to_owned(),
        "Sorry, you are not allowed to create posts as this user.".to_owned(),
        401,
    );
    let unavailable = WordPressError::unavailable(
        "WordPress site https://example.test is unavailable: connection refused",
    );

    // Act
    let rendered = [
        insecure.to_string(),
        refused_without_detail.to_string(),
        refused_with_detail.to_string(),
        response.to_string(),
        unavailable.to_string(),
    ];

    // Assert
    assert_eq!(
        rendered,
        [
            "refusing to send WordPress credentials to a non-https URL: http://example.test/wp-json/"
                .to_owned(),
            "WordPress rejected the credentials for https://example.test/wp-json/ (HTTP 403)"
                .to_owned(),
            "WordPress rejected the credentials for https://example.test/wp-json/ (HTTP 401): invalid_username: Unknown username."
                .to_owned(),
            "WordPress error rest_cannot_create (HTTP 401): Sorry, you are not allowed to create posts as this user."
                .to_owned(),
            "WordPress site https://example.test is unavailable: connection refused".to_owned(),
        ]
    );
}

#[test]
fn test_response_error_carries_code_message_status() {
    // Arrange
    let error = WordPressError::Response {
        code: "rest_forbidden".to_owned(),
        message: "Sorry, you are not allowed to do that.".to_owned(),
        status: 403,
    };

    // Act
    let rendered = error.to_string();

    // Assert
    assert_eq!(
        error,
        WordPressError::Response {
            code: "rest_forbidden".to_owned(),
            message: "Sorry, you are not allowed to do that.".to_owned(),
            status: 403,
        }
    );
    assert_eq!(
        rendered,
        "WordPress error rest_forbidden (HTTP 403): Sorry, you are not allowed to do that."
    );
}

#[test]
fn test_insecure_names_the_refused_url() {
    // Arrange
    let url = "ftp://example.test/blog";

    // Act
    let error = WordPressError::insecure(url);

    // Assert
    assert_eq!(
        error,
        WordPressError::InsecureUrl {
            url: "ftp://example.test/blog".to_owned(),
        }
    );
}

#[test]
fn test_refused_with_detail_appends_the_code_and_message() {
    // Arrange
    let detail = Some(("rest_forbidden".to_owned(), "Sorry.".to_owned()));

    // Act
    let error = WordPressError::refused("https://example.test/", 403, detail);

    // Assert
    assert_eq!(
        error,
        WordPressError::Auth {
            url: "https://example.test/".to_owned(),
            status: 403,
            detail: ": rest_forbidden: Sorry.".to_owned(),
        }
    );
}

#[test]
fn test_refused_without_detail_leaves_the_suffix_empty() {
    // Arrange
    let detail = None;

    // Act
    let error = WordPressError::refused("https://example.test/", 401, detail);

    // Assert
    assert_eq!(
        error,
        WordPressError::Auth {
            url: "https://example.test/".to_owned(),
            status: 401,
            detail: String::new(),
        }
    );
}

#[test]
fn test_unavailable_carries_the_message() {
    // Arrange
    let message = "WordPress site https://example.test did not return JSON: expected value";

    // Act
    let error = WordPressError::unavailable(message);

    // Assert
    assert_eq!(
        error,
        WordPressError::Unavailable {
            message: "WordPress site https://example.test did not return JSON: expected value"
                .to_owned(),
        }
    );
}
