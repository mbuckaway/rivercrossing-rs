// SPDX-License-Identifier: GPL-3.0-only

//! Tests for REST-index discovery.

use pretty_assertions::assert_eq;

use super::{SiteInfo, discover_with};
use crate::auth::BasicAuth;
use crate::http::{HttpRequest, Method};
use crate::mock::MockTransport;

const BASE: &str = "https://example.test";
const INDEX_URL: &str = "https://example.test/wp-json/";

/// Return the credentials that every discovery test sends.
fn auth() -> BasicAuth {
    BasicAuth::new("user", "password")
}

/// Return an index with no namespaces and no authentication mapping.
fn empty_site() -> SiteInfo {
    SiteInfo {
        namespaces: Vec::new(),
        application_passwords: false,
    }
}

#[test]
fn test_discover_requests_the_rest_index_as_a_get_with_credentials() {
    // Arrange
    let transport = MockTransport::answering(200, r#"{"namespaces": ["wp/v2"]}"#);

    // Act
    let result = discover_with(&transport, BASE, &auth());

    // Assert
    assert_eq!(
        result,
        Ok(SiteInfo {
            namespaces: vec!["wp/v2".to_owned()],
            application_passwords: false,
        })
    );
    assert_eq!(
        transport.calls(),
        vec![HttpRequest {
            method: Method::Get,
            url: INDEX_URL.to_owned(),
            headers: vec![
                (
                    "Authorization".to_owned(),
                    "Basic dXNlcjpwYXNzd29yZA==".to_owned(),
                ),
                ("Accept".to_owned(), "application/json".to_owned()),
            ],
            body: None,
        }]
    );
}

#[test]
fn test_discover_parses_namespaces_and_application_password_support() {
    // Arrange
    let transport = MockTransport::answering(
        200,
        r#"{"namespaces": ["wp/v2", "wp/v2/posts"], "authentication": {"application-passwords": {"endpoints": []}}}"#,
    );

    // Act
    let result = discover_with(&transport, BASE, &auth());

    // Assert
    assert_eq!(
        result,
        Ok(SiteInfo {
            namespaces: vec!["wp/v2".to_owned(), "wp/v2/posts".to_owned()],
            application_passwords: true,
        })
    );
}

#[test]
fn test_discover_given_missing_or_mixed_namespaces() {
    // Arrange
    let absent = MockTransport::answering(200, r#"{"name": "Race results"}"#);
    let empty = MockTransport::answering(200, r#"{"namespaces": []}"#);
    let mixed = MockTransport::answering(200, r#"{"namespaces": [1, "wp/v2", true]}"#);

    // Act
    let results = [
        discover_with(&absent, BASE, &auth()),
        discover_with(&empty, BASE, &auth()),
        discover_with(&mixed, BASE, &auth()),
    ];

    // Assert
    assert_eq!(
        results,
        [
            Ok(empty_site()),
            Ok(empty_site()),
            Ok(SiteInfo {
                namespaces: vec!["wp/v2".to_owned()],
                application_passwords: false,
            }),
        ]
    );
}

#[test]
fn test_discover_given_the_authentication_mapping() {
    // Arrange
    let absent = MockTransport::answering(200, r#"{"name": "Race results"}"#);
    let empty = MockTransport::answering(200, r#"{"authentication": {}}"#);
    let present = MockTransport::answering(
        200,
        r#"{"authentication": {"application-passwords": {"endpoints": []}}}"#,
    );
    let other_key = MockTransport::answering(200, r#"{"authentication": {"oauth1": {}}}"#);
    let not_an_object = MockTransport::answering(200, r#"{"authentication": []}"#);

    // Act
    let results = [
        discover_with(&absent, BASE, &auth()),
        discover_with(&empty, BASE, &auth()),
        discover_with(&present, BASE, &auth()),
        discover_with(&other_key, BASE, &auth()),
        discover_with(&not_an_object, BASE, &auth()),
    ];

    // Assert
    assert_eq!(
        results,
        [
            Ok(empty_site()),
            Ok(empty_site()),
            Ok(SiteInfo {
                namespaces: Vec::new(),
                application_passwords: true,
            }),
            Ok(empty_site()),
            Ok(empty_site()),
        ]
    );
}

#[test]
fn test_discover_maps_401_with_body_to_auth_error() {
    // Arrange
    let transport =
        MockTransport::answering(401, r#"{"code": "rest_forbidden", "message": "Sorry"}"#);

    // Act
    let result = discover_with(&transport, BASE, &auth()).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err(
            "WordPress rejected the credentials for https://example.test/wp-json/ (HTTP 401): rest_forbidden: Sorry"
                .to_owned()
        )
    );
}

#[test]
fn test_discover_maps_403_without_body_to_auth_error() {
    // Arrange
    let transport = MockTransport::answering(403, "");

    // Act
    let result = discover_with(&transport, BASE, &auth()).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err(
            "WordPress rejected the credentials for https://example.test/wp-json/ (HTTP 403)"
                .to_owned()
        )
    );
}

#[test]
fn test_discover_maps_500_with_body_to_response_error() {
    // Arrange
    let transport = MockTransport::answering(
        500,
        r#"{"code": "internal_server_error", "message": "Boom"}"#,
    );

    // Act
    let result = discover_with(&transport, BASE, &auth()).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err("WordPress error internal_server_error (HTTP 500): Boom".to_owned())
    );
}

#[test]
fn test_discover_maps_404_without_body_to_unavailable() {
    // Arrange
    let transport = MockTransport::answering(404, r#"{"message": "no code here"}"#);

    // Act
    let result = discover_with(&transport, BASE, &auth()).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err(
            "WordPress site https://example.test/wp-json/ returned HTTP 404 without a JSON error body"
                .to_owned()
        )
    );
}

#[test]
fn test_discover_maps_malformed_error_body_to_unavailable() {
    // Arrange
    let array_body = MockTransport::answering(500, r#"["code", "message"]"#);
    let integer_code = MockTransport::answering(500, r#"{"code": 1, "message": "m"}"#);
    let integer_message = MockTransport::answering(500, r#"{"code": "c", "message": 2}"#);
    let missing_message = MockTransport::answering(500, r#"{"code": "c"}"#);

    // Act
    let results = [
        discover_with(&array_body, BASE, &auth()),
        discover_with(&integer_code, BASE, &auth()),
        discover_with(&integer_message, BASE, &auth()),
        discover_with(&missing_message, BASE, &auth()),
    ]
    .map(|result| result.map_err(|error| error.to_string()));

    // Assert
    assert_eq!(
        results,
        [
            Err(
                "WordPress site https://example.test/wp-json/ returned HTTP 500 without a JSON error body"
                    .to_owned()
            ),
            Err(
                "WordPress site https://example.test/wp-json/ returned HTTP 500 without a JSON error body"
                    .to_owned()
            ),
            Err(
                "WordPress site https://example.test/wp-json/ returned HTTP 500 without a JSON error body"
                    .to_owned()
            ),
            Err(
                "WordPress site https://example.test/wp-json/ returned HTTP 500 without a JSON error body"
                    .to_owned()
            ),
        ]
    );
}

#[test]
fn test_discover_given_invalid_utf8_returns_unavailable() {
    // Arrange
    let transport = MockTransport::answering_bytes(200, vec![0x7b, 0xff, 0x7d]);
    let prefix = "WordPress site https://example.test/wp-json/ did not return JSON: ";

    // Act
    let result = discover_with(&transport, BASE, &auth()).map_err(|error| error.to_string());

    // Assert
    assert!(matches!(result, Err(ref text) if text.starts_with(prefix)));
}

#[test]
fn test_discover_maps_non_object_to_unavailable() {
    // Arrange
    let transport = MockTransport::answering(200, "[]");

    // Act
    let result = discover_with(&transport, BASE, &auth()).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err("WordPress site https://example.test/wp-json/ did not return a JSON object".to_owned())
    );
}

#[test]
fn test_discover_maps_a_transport_failure_to_unavailable() {
    // Arrange
    let transport = MockTransport::failing("connection refused");

    // Act
    let result = discover_with(&transport, BASE, &auth()).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err(
            "WordPress site https://example.test/wp-json/ is unavailable: connection refused"
                .to_owned()
        )
    );
}

#[test]
fn test_discover_refuses_a_non_https_base_url_before_sending() {
    // Arrange
    let base = "http://example.test";

    // Act
    let result = super::discover(base, &auth()).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err(
            "refusing to send WordPress credentials to a non-https URL: http://example.test/wp-json/"
                .to_owned()
        )
    );
}
