// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the `WordPress` Application Password header.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use pretty_assertions::assert_eq;

use super::BasicAuth;

/// Return the header that `credentials` should encode to.
fn expected_header(credentials: &str) -> String {
    format!("Basic {}", STANDARD.encode(credentials))
}

#[test]
fn test_basic_auth_header_value_encodes_credentials_without_chunk_spaces() {
    // Arrange
    let solid = BasicAuth::new("user", "abcdefghijkl");
    let chunked = BasicAuth::new("user", "abcd efgh ijkl");
    let doubled = BasicAuth::new("user", "abcd  efgh");
    let only_spaces = BasicAuth::new("user", "   ");
    let empty = BasicAuth::new("user", "");
    let spaced_user = BasicAuth::new("us er", "abcd efgh");

    // Act
    let headers = [
        solid.header_value(),
        chunked.header_value(),
        doubled.header_value(),
        only_spaces.header_value(),
        empty.header_value(),
        spaced_user.header_value(),
    ];

    // Assert
    assert_eq!(
        headers,
        [
            expected_header("user:abcdefghijkl"),
            expected_header("user:abcdefghijkl"),
            expected_header("user:abcdefgh"),
            expected_header("user:"),
            expected_header("user:"),
            expected_header("us er:abcdefgh"),
        ]
    );
}

#[test]
fn test_basic_auth_header_matches_independent_base64() {
    // Arrange
    let auth = BasicAuth::new("user", "password");

    // Act
    let header = auth.header_value();

    // Assert
    assert_eq!(header, "Basic dXNlcjpwYXNzd29yZA==");
}

#[test]
fn test_basic_auth_debug_redacts_the_password() {
    // Arrange
    let auth = BasicAuth::new("user", "s3cret application password");

    // Act
    let rendered = format!("{auth:?}");

    // Assert
    assert_eq!(
        rendered,
        "BasicAuth { username: \"user\", password: \"<redacted>\" }"
    );
}

#[test]
fn test_basic_auth_given_equal_credentials_compares_equal() {
    // Arrange
    let from_str = BasicAuth::new("user", "password");
    let from_string = BasicAuth::new(String::from("user"), String::from("password"));

    // Act
    let cloned = from_str.clone();

    // Assert
    assert_eq!(cloned, from_string);
}
