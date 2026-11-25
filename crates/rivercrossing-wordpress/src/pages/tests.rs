// SPDX-License-Identifier: GPL-3.0-only

//! Tests for page lookup and publish.

use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use super::{PageDraft, PublishedPage, find_page_by_slug_with, publish_page_with};
use crate::auth::BasicAuth;
use crate::discover::discover_with;
use crate::http::{HttpRequest, Method};
use crate::mock::MockTransport;

const BASE: &str = "https://example.test";
const PAGES_URL: &str = "https://example.test/wp-json/wp/v2/pages";
const RESULTS_URL: &str = "https://example.test/wp-json/wp/v2/pages?slug=results&context=edit";

/// Return the credentials that every page test sends.
fn auth() -> BasicAuth {
    BasicAuth::new("user", "password")
}

/// Return the page that every publish test sends.
fn draft() -> PageDraft<'static> {
    PageDraft::new("Results", "results", "<p>Hello</p>", "draft")
}

/// Return the headers of a request that carries no body.
fn page_headers() -> Vec<(String, String)> {
    vec![
        (
            "Authorization".to_owned(),
            "Basic dXNlcjpwYXNzd29yZA==".to_owned(),
        ),
        ("Accept".to_owned(), "application/json".to_owned()),
    ]
}

/// Return the headers of a request that carries a JSON body.
fn page_post_headers() -> Vec<(String, String)> {
    let mut headers = page_headers();
    headers.push((
        "Content-Type".to_owned(),
        "application/json; charset=utf-8".to_owned(),
    ));
    headers
}

/// Return the GET request that a slug query sends.
fn slug_query(url: &str) -> HttpRequest {
    HttpRequest {
        method: Method::Get,
        url: url.to_owned(),
        headers: page_headers(),
        body: None,
    }
}

/// Return the recorded calls with their bodies stripped.
fn envelopes(calls: &[HttpRequest]) -> Vec<HttpRequest> {
    calls
        .iter()
        .map(|call| HttpRequest {
            body: None,
            ..call.clone()
        })
        .collect()
}

/// Return the recorded call's body, parsed back into JSON.
fn body_json(calls: &[HttpRequest]) -> Option<Value> {
    calls
        .first()?
        .body
        .as_deref()
        .and_then(|body| serde_json::from_slice(body).ok())
}

#[test]
fn test_find_page_by_slug_returns_the_first_id() {
    // Arrange
    let transport = MockTransport::answering(200, r#"[{"id": 7}, {"id": 9}]"#);

    // Act
    let result = find_page_by_slug_with(&transport, BASE, &auth(), "results");

    // Assert
    assert_eq!(result, Ok(Some(7)));
    assert_eq!(transport.calls(), vec![slug_query(RESULTS_URL)]);
}

#[test]
fn test_find_page_by_slug_queries_slug_and_edit_context() {
    // Arrange
    let empty_slug = MockTransport::answering(200, "[]");
    let spaced_slug = MockTransport::answering(200, "[]");
    let empty_url = "https://example.test/wp-json/wp/v2/pages?slug=&context=edit";
    let spaced_url = "https://example.test/wp-json/wp/v2/pages?slug=a+team+page&context=edit";

    // Act
    let results = [
        find_page_by_slug_with(&empty_slug, BASE, &auth(), ""),
        find_page_by_slug_with(&spaced_slug, BASE, &auth(), "a team page"),
    ];

    // Assert
    assert_eq!(results, [Ok(None), Ok(None)]);
    assert_eq!(empty_slug.calls(), vec![slug_query(empty_url)]);
    assert_eq!(spaced_slug.calls(), vec![slug_query(spaced_url)]);
}

#[test]
fn test_find_page_by_slug_returns_none_when_empty() {
    // Arrange
    let transport = MockTransport::answering(200, "[]");

    // Act
    let result = find_page_by_slug_with(&transport, BASE, &auth(), "results");

    // Assert
    assert_eq!(result, Ok(None));
}

#[test]
fn test_find_page_by_slug_requires_an_array() {
    // Arrange
    let transport = MockTransport::answering(200, "{}");

    // Act
    let result = find_page_by_slug_with(&transport, BASE, &auth(), "results")
        .map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err(
            "WordPress site https://example.test/wp-json/wp/v2/pages?slug=results&context=edit did not return a JSON array"
                .to_owned()
        )
    );
}

#[test]
fn test_find_page_by_slug_given_a_missing_or_non_integer_id() {
    // Arrange
    let missing = MockTransport::answering(200, "[{}]");
    let string_id = MockTransport::answering(200, r#"[{"id": "7"}]"#);
    let boolean_id = MockTransport::answering(200, r#"[{"id": true}]"#);
    let expected = Err(
        "WordPress site https://example.test/wp-json/wp/v2/pages?slug=results&context=edit returned a response without an integer id"
            .to_owned(),
    );

    // Act
    let results = [
        find_page_by_slug_with(&missing, BASE, &auth(), "results"),
        find_page_by_slug_with(&string_id, BASE, &auth(), "results"),
        find_page_by_slug_with(&boolean_id, BASE, &auth(), "results"),
    ]
    .map(|result| result.map_err(|error| error.to_string()));

    // Assert
    assert_eq!(results, [expected.clone(), expected.clone(), expected]);
}

#[test]
fn test_find_page_by_slug_given_a_non_object_page() {
    // Arrange
    let transport = MockTransport::answering(200, "[[]]");

    // Act
    let result = find_page_by_slug_with(&transport, BASE, &auth(), "results")
        .map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err(
            "WordPress site https://example.test/wp-json/wp/v2/pages?slug=results&context=edit returned a response without an integer id"
                .to_owned()
        )
    );
}

#[test]
fn test_publish_page_creates_when_no_page_id() {
    // Arrange
    let transport = MockTransport::answering(
        200,
        r#"{"id": 12, "link": "https://example.test/?page_id=12", "status": "draft"}"#,
    );

    // Act
    let result = publish_page_with(&transport, BASE, &auth(), &draft());

    // Assert
    assert_eq!(
        result,
        Ok(PublishedPage {
            id: 12,
            link: "https://example.test/?page_id=12".to_owned(),
            status: "draft".to_owned(),
            created: true,
        })
    );
    assert_eq!(
        envelopes(&transport.calls()),
        vec![HttpRequest {
            method: Method::Post,
            url: PAGES_URL.to_owned(),
            headers: page_post_headers(),
            body: None,
        }]
    );
    assert_eq!(
        body_json(&transport.calls()),
        Some(json!({
            "title": "Results",
            "slug": "results",
            "status": "draft",
            "parent": 0,
            "content": "<p>Hello</p>",
        }))
    );
}

#[test]
fn test_publish_page_updates_when_page_id_given() {
    // Arrange
    let mut draft = PageDraft::new("Results", "results", "<p>Hello</p>", "draft");
    draft.page_id = Some(12);
    let transport = MockTransport::answering(
        200,
        r#"{"id": 12, "link": "https://example.test/?page_id=12", "status": "draft"}"#,
    );

    // Act
    let result = publish_page_with(&transport, BASE, &auth(), &draft);

    // Assert
    assert_eq!(
        result,
        Ok(PublishedPage {
            id: 12,
            link: "https://example.test/?page_id=12".to_owned(),
            status: "draft".to_owned(),
            created: false,
        })
    );
    assert_eq!(
        envelopes(&transport.calls()),
        vec![HttpRequest {
            method: Method::Post,
            url: "https://example.test/wp-json/wp/v2/pages/12".to_owned(),
            headers: page_post_headers(),
            body: None,
        }]
    );
}

#[test]
fn test_publish_page_sends_bare_html_content() {
    // Arrange
    let draft = PageDraft::new(
        "Results",
        "results",
        r#"<p class="lead">Hello &amp; welcome</p>"#,
        "draft",
    );
    let transport = MockTransport::answering(200, r#"{"id": 3, "link": "l", "status": "draft"}"#);

    // Act
    let result = publish_page_with(&transport, BASE, &auth(), &draft);

    // Assert
    assert_eq!(result.map(|page| page.id), Ok(3));
    assert_eq!(
        body_json(&transport.calls()),
        Some(json!({
            "title": "Results",
            "slug": "results",
            "status": "draft",
            "parent": 0,
            "content": r#"<p class="lead">Hello &amp; welcome</p>"#,
        }))
    );
}

#[test]
fn test_publish_page_reports_the_site_status() {
    // Arrange
    let transport = MockTransport::answering(
        200,
        r#"{"id": 5, "link": "https://example.test/?page_id=5", "status": "publish"}"#,
    );

    // Act
    let result = publish_page_with(&transport, BASE, &auth(), &draft());

    // Assert
    assert_eq!(
        result,
        Ok(PublishedPage {
            id: 5,
            link: "https://example.test/?page_id=5".to_owned(),
            status: "publish".to_owned(),
            created: true,
        })
    );
}

#[test]
fn test_publish_page_given_an_incomplete_response() {
    // Arrange
    let missing_id = MockTransport::answering(200, "{}");
    let missing_link = MockTransport::answering(200, r#"{"id": 1}"#);
    let missing_status = MockTransport::answering(200, r#"{"id": 1, "link": "l"}"#);
    let url = "https://example.test/wp-json/wp/v2/pages";

    // Act
    let results = [
        publish_page_with(&missing_id, BASE, &auth(), &draft()),
        publish_page_with(&missing_link, BASE, &auth(), &draft()),
        publish_page_with(&missing_status, BASE, &auth(), &draft()),
    ]
    .map(|result| result.map_err(|error| error.to_string()));

    // Assert
    assert_eq!(
        results,
        [
            Err(format!(
                "WordPress site {url} returned a response without an integer id"
            )),
            Err(format!(
                "WordPress site {url} returned a response without a 'link' field"
            )),
            Err(format!(
                "WordPress site {url} returned a response without a 'status' field"
            )),
        ]
    );
}

#[test]
fn test_publish_page_rejects_a_non_object() {
    // Arrange
    let transport = MockTransport::answering(200, "[]");

    // Act
    let result =
        publish_page_with(&transport, BASE, &auth(), &draft()).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err(
            "WordPress site https://example.test/wp-json/wp/v2/pages did not return a JSON object"
                .to_owned()
        )
    );
}

#[test]
fn test_publish_page_maps_403_to_auth_error() {
    // Arrange
    let transport =
        MockTransport::answering(403, r#"{"code": "rest_forbidden", "message": "Sorry"}"#);

    // Act
    let result =
        publish_page_with(&transport, BASE, &auth(), &draft()).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        result,
        Err(
            "WordPress rejected the credentials for https://example.test/wp-json/wp/v2/pages (HTTP 403): rest_forbidden: Sorry"
                .to_owned()
        )
    );
}

#[test]
fn test_every_request_refuses_a_non_https_base_url() {
    // Arrange
    let http_base = "http://example.test";
    let discover_mock = MockTransport::default();
    let find_mock = MockTransport::default();
    let publish_mock = MockTransport::default();

    // Act
    let results = [
        discover_with(&discover_mock, http_base, &auth()).map(|_info| ()),
        find_page_by_slug_with(&find_mock, http_base, &auth(), "results").map(|_page| ()),
        publish_page_with(&publish_mock, http_base, &auth(), &draft()).map(|_page| ()),
    ]
    .map(|result| result.map_err(|error| error.to_string()));

    // Assert
    assert_eq!(
        results,
        [
            Err(
                "refusing to send WordPress credentials to a non-https URL: http://example.test/wp-json/"
                    .to_owned()
            ),
            Err(
                "refusing to send WordPress credentials to a non-https URL: http://example.test/wp-json/wp/v2/pages?slug=results&context=edit"
                    .to_owned()
            ),
            Err(
                "refusing to send WordPress credentials to a non-https URL: http://example.test/wp-json/wp/v2/pages"
                    .to_owned()
            ),
        ]
    );
    assert_eq!(
        [
            discover_mock.calls(),
            find_mock.calls(),
            publish_mock.calls()
        ],
        [Vec::new(), Vec::new(), Vec::new()]
    );
}

#[test]
fn test_public_page_entry_points_refuse_a_non_https_base_url() {
    // Arrange
    let base = "http://example.test";

    // Act
    let found = super::find_page_by_slug(base, &auth(), "results").map(|_page| ());
    let published = super::publish_page(base, &auth(), &draft()).map(|_page| ());

    // Assert
    assert_eq!(
        [found, published].map(|result| result.map_err(|error| error.to_string())),
        [
            Err(
                "refusing to send WordPress credentials to a non-https URL: http://example.test/wp-json/wp/v2/pages?slug=results&context=edit"
                    .to_owned()
            ),
            Err(
                "refusing to send WordPress credentials to a non-https URL: http://example.test/wp-json/wp/v2/pages"
                    .to_owned()
            ),
        ]
    );
}
