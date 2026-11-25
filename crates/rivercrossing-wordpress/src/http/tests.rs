// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the HTTP seam, the scheme gate and the URL helpers.

use pretty_assertions::assert_eq;
use serde_json::json;

use super::{
    HttpRequest, Method, build_request, request_json_with, require_https, rest_url, scheme_is_https,
};
use crate::auth::BasicAuth;
use crate::error::WordPressError;
use crate::mock::MockTransport;

#[test]
fn test_rest_url_strips_all_trailing_slashes() {
    // Arrange
    let bare = "https://x";
    let slashes = "https://x///";

    // Act
    let from_bare = rest_url(bare, "wp-json/", &[]);
    let from_slashes = rest_url(slashes, "wp-json/", &[]);

    // Assert
    assert_eq!(
        [from_bare, from_slashes],
        [
            "https://x/wp-json/".to_owned(),
            "https://x/wp-json/".to_owned()
        ]
    );
}

#[test]
fn test_rest_url_keeps_a_subdirectory_base() {
    // Arrange
    let base = "https://x/blog/";
    let path = "wp-json/wp/v2/pages";
    let params = [("slug", "results"), ("context", "edit")];

    // Act
    let url = rest_url(base, path, &params);

    // Assert
    assert_eq!(
        url,
        "https://x/blog/wp-json/wp/v2/pages?slug=results&context=edit"
    );
}

#[test]
fn test_rest_url_percent_encodes_the_query_in_order() {
    // Arrange
    let path = "wp-json/wp/v2/pages";

    // Act
    let spaced = rest_url(
        "https://x",
        path,
        &[("slug", "a team page"), ("context", "edit")],
    );
    let empty = rest_url("https://x", path, &[("slug", ""), ("context", "edit")]);
    let reserved = rest_url("https://x", path, &[("slug", "a&b=c")]);

    // Assert
    assert_eq!(
        [spaced, empty, reserved],
        [
            "https://x/wp-json/wp/v2/pages?slug=a+team+page&context=edit".to_owned(),
            "https://x/wp-json/wp/v2/pages?slug=&context=edit".to_owned(),
            "https://x/wp-json/wp/v2/pages?slug=a%26b%3Dc".to_owned(),
        ]
    );
}

#[test]
fn test_scheme_is_https_matches_urlsplit() {
    // Arrange
    let urls = [
        "https://x",
        "HTTPS://x",
        "HtTpS://x",
        "http://x",
        "ftp://x",
        "//x",
        "example.test/x",
        "1https://x",
        "ht tps://x",
        "https",
        ":x",
        "https+://x",
    ];

    // Act
    let verdicts = urls.map(scheme_is_https);

    // Assert
    assert_eq!(
        verdicts,
        [
            true, true, true, false, false, false, false, false, false, false, false, false,
        ]
    );
}

#[test]
fn test_require_https_accepts_an_https_url() {
    // Arrange
    let url = "HTTPS://example.test/wp-json/";

    // Act
    let result = require_https(url);

    // Assert
    assert_eq!(result, Ok(()));
}

#[test]
fn test_require_https_refuses_a_plain_http_url() {
    // Arrange
    let url = "http://example.test/wp-json/";

    // Act
    let result = require_https(url);

    // Assert
    assert_eq!(
        result,
        Err(WordPressError::InsecureUrl {
            url: "http://example.test/wp-json/".to_owned(),
        })
    );
}

#[test]
fn test_build_request_attaches_credentials_and_json_headers() {
    // Arrange
    let auth = BasicAuth::new("user", "password");
    let get_url = "https://x/wp-json/";
    let post_url = "https://x/wp-json/wp/v2/pages";
    let payload = b"{\"title\":\"Race results\"}".to_vec();

    // Act
    let get = build_request(get_url, &auth, None);
    let post = build_request(post_url, &auth, Some(payload.clone()));

    // Assert
    assert_eq!(
        get,
        Ok(HttpRequest {
            method: Method::Get,
            url: "https://x/wp-json/".to_owned(),
            headers: vec![
                (
                    "Authorization".to_owned(),
                    "Basic dXNlcjpwYXNzd29yZA==".to_owned(),
                ),
                ("Accept".to_owned(), "application/json".to_owned()),
            ],
            body: None,
        })
    );
    assert_eq!(
        post,
        Ok(HttpRequest {
            method: Method::Post,
            url: "https://x/wp-json/wp/v2/pages".to_owned(),
            headers: vec![
                (
                    "Authorization".to_owned(),
                    "Basic dXNlcjpwYXNzd29yZA==".to_owned(),
                ),
                ("Accept".to_owned(), "application/json".to_owned()),
                (
                    "Content-Type".to_owned(),
                    "application/json; charset=utf-8".to_owned(),
                ),
            ],
            body: Some(payload),
        })
    );
}

#[test]
fn test_build_request_refuses_a_non_https_url() {
    // Arrange
    let auth = BasicAuth::new("user", "password");
    let url = "http://example.test/wp-json/";

    // Act
    let result = build_request(url, &auth, Some(b"{}".to_vec()));

    // Assert
    assert_eq!(
        result.map_err(|error| error.to_string()),
        Err(
            "refusing to send WordPress credentials to a non-https URL: http://example.test/wp-json/"
                .to_owned()
        )
    );
}

#[test]
fn test_request_json_maps_401_with_and_without_a_body() {
    // Arrange
    let auth = BasicAuth::new("user", "password");
    let with_body =
        MockTransport::answering(401, r#"{"code": "rest_forbidden", "message": "Sorry"}"#);
    let without_body = MockTransport::answering(401, "not json");

    // Act
    let detailed = request_json_with(&with_body, "https://example.test/wp-json/", &auth, None)
        .map_err(|error| error.to_string());
    let plain = request_json_with(&without_body, "https://example.test/wp-json/", &auth, None)
        .map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        [detailed, plain],
        [
            Err(
                "WordPress rejected the credentials for https://example.test/wp-json/ (HTTP 401): rest_forbidden: Sorry"
                    .to_owned()
            ),
            Err(
                "WordPress rejected the credentials for https://example.test/wp-json/ (HTTP 401)"
                    .to_owned()
            ),
        ]
    );
}

#[test]
fn test_request_json_maps_500_with_a_body() {
    // Arrange
    let auth = BasicAuth::new("user", "password");
    let transport = MockTransport::answering(
        500,
        r#"{"code": "internal_server_error", "message": "Boom"}"#,
    );

    // Act
    let result = request_json_with(&transport, "https://example.test/wp-json/", &auth, None)
        .map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err("WordPress error internal_server_error (HTTP 500): Boom".to_owned())
    );
}

#[test]
fn test_request_json_maps_other_status_without_a_body() {
    // Arrange
    let auth = BasicAuth::new("user", "password");
    let transport = MockTransport::answering(404, r#"["not", "an", "error", "body"]"#);

    // Act
    let result = request_json_with(&transport, "https://example.test/wp-json/", &auth, None)
        .map_err(|error| error.to_string());

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
fn test_request_json_maps_a_transport_failure() {
    // Arrange
    let auth = BasicAuth::new("user", "password");
    let transport = MockTransport::failing("connection refused");

    // Act
    let result = request_json_with(&transport, "https://example.test/wp-json/", &auth, None)
        .map_err(|error| error.to_string());

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
fn test_request_json_posts_the_body_when_present() {
    // Arrange
    let auth = BasicAuth::new("user", "password");
    let transport = MockTransport::answering(200, r#"{"id": 7}"#);
    let url = "https://example.test/wp-json/wp/v2/pages";
    let payload = b"{\"title\":\"Race results\"}".to_vec();

    // Act
    let result = request_json_with(&transport, url, &auth, Some(payload.clone()));

    // Assert
    assert_eq!(result, Ok(json!({"id": 7})));
    assert_eq!(
        transport.calls(),
        vec![HttpRequest {
            method: Method::Post,
            url: "https://example.test/wp-json/wp/v2/pages".to_owned(),
            headers: vec![
                (
                    "Authorization".to_owned(),
                    "Basic dXNlcjpwYXNzd29yZA==".to_owned(),
                ),
                ("Accept".to_owned(), "application/json".to_owned()),
                (
                    "Content-Type".to_owned(),
                    "application/json; charset=utf-8".to_owned(),
                ),
            ],
            body: Some(payload),
        }]
    );
}

#[test]
fn test_request_json_refuses_a_non_https_url_without_sending() {
    // Arrange
    let auth = BasicAuth::new("user", "password");
    let transport = MockTransport::answering(200, "{}");

    // Act
    let result = request_json_with(&transport, "http://example.test/wp-json/", &auth, None)
        .map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err(
            "refusing to send WordPress credentials to a non-https URL: http://example.test/wp-json/"
                .to_owned()
        )
    );
    assert_eq!(transport.calls(), Vec::new());
}
