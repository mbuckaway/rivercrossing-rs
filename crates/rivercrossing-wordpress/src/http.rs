// SPDX-License-Identifier: GPL-3.0-only

//! The crate-private HTTP seam and the URL helpers that every request uses.

use serde_json::Value;
use url::form_urlencoded::Serializer;

use crate::auth::BasicAuth;
use crate::error::WordPressError;
use crate::json::{decode_json, error_body};

/// The HTTP method that the crate sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Method {
    /// A read with no request body.
    Get,
    /// A write with a JSON body.
    Post,
}

/// One request handed to the [`Transport`] seam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HttpRequest {
    /// The method.
    pub(crate) method: Method,
    /// The absolute URL.
    pub(crate) url: String,
    /// The headers, in send order.
    pub(crate) headers: Vec<(String, String)>,
    /// The JSON body, when the request carries one.
    pub(crate) body: Option<Vec<u8>>,
}

/// One response from the [`Transport`] seam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HttpResponse {
    /// The HTTP status that the site answered with.
    pub(crate) status: u16,
    /// The response body bytes.
    pub(crate) body: Vec<u8>,
}

/// The HTTP boundary.
///
/// The unit tests substitute a recording mock; the production implementation
/// is `transport::UreqTransport`.
pub(crate) trait Transport {
    /// Send one request and return its status and body.
    ///
    /// # Errors
    ///
    /// Return `Err` with the transport's failure reason when no response
    /// arrived: a connection, TLS, timeout or body-limit failure.
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, String>;
}

/// Return true when the URL's scheme is `https`, case-insensitively.
///
/// The check mirrors `urllib.parse.urlsplit`: the text before the first `:`
/// is a scheme only when it starts with an ASCII letter and holds only ASCII
/// alphanumerics, `+`, `-` and `.`. A URL with no scheme is not `https`, so
/// the credential gate never trusts a `Url::parse` failure.
pub(crate) fn scheme_is_https(url: &str) -> bool {
    let Some((scheme, _rest)) = url.split_once(':') else {
        return false;
    };
    let mut characters = scheme.chars();
    if !characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
    {
        return false;
    }
    if !characters
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.'))
    {
        return false;
    }
    scheme.eq_ignore_ascii_case("https")
}

/// Return `Ok(())` for an `https` URL and [`WordPressError::InsecureUrl`]
/// otherwise.
///
/// # Errors
///
/// Return [`WordPressError::InsecureUrl`] when [`scheme_is_https`] is false.
pub(crate) fn require_https(url: &str) -> Result<(), WordPressError> {
    if scheme_is_https(url) {
        return Ok(());
    }
    Err(WordPressError::insecure(url))
}

/// Return `{base without trailing '/'}/{path}`, plus the encoded query.
///
/// The pairs keep their order, and the encoding matches Python's
/// `urlencode`: a space becomes `+`.
pub(crate) fn rest_url(base_url: &str, path: &str, params: &[(&str, &str)]) -> String {
    let base = base_url.trim_end_matches('/');
    let mut url = format!("{base}/{path}");
    if !params.is_empty() {
        let query = Serializer::new(String::new())
            .extend_pairs(params.iter().copied())
            .finish();
        url.push('?');
        url.push_str(&query);
    }
    url
}

/// Return the request that the oracle's `_build_request` builds.
///
/// The HTTPS gate runs first, so a refused URL never reaches a header. The
/// `Authorization` header comes first, then `Accept`, and `Content-Type` only
/// on a request that carries a body.
///
/// # Errors
///
/// Return [`WordPressError::InsecureUrl`] for a URL that is not `https`.
pub(crate) fn build_request(
    url: &str,
    auth: &BasicAuth,
    body: Option<Vec<u8>>,
) -> Result<HttpRequest, WordPressError> {
    require_https(url)?;
    let mut headers = vec![
        ("Authorization".to_owned(), auth.header_value()),
        ("Accept".to_owned(), "application/json".to_owned()),
    ];
    let Some(body) = body else {
        return Ok(HttpRequest {
            method: Method::Get,
            url: url.to_owned(),
            headers,
            body: None,
        });
    };
    headers.push((
        "Content-Type".to_owned(),
        "application/json; charset=utf-8".to_owned(),
    ));
    Ok(HttpRequest {
        method: Method::Post,
        url: url.to_owned(),
        headers,
        body: Some(body),
    })
}

/// Return the JSON document that one request to `url` returns.
///
/// The HTTPS gate runs before the credential header is built. A transport
/// failure becomes [`WordPressError::Unavailable`], an HTTP error status maps
/// through the status mapper, and any other status decodes the body as JSON.
///
/// # Errors
///
/// Return [`WordPressError::InsecureUrl`] for a non-https URL, and the mapped
/// error for a failed transport, an HTTP error status or a non-JSON body.
#[expect(
    clippy::too_many_arguments,
    reason = "the pipeline mirrors the oracle's _request_json(transport, url, auth, body)"
)]
pub(crate) fn request_json_with(
    transport: &dyn Transport,
    url: &str,
    auth: &BasicAuth,
    body: Option<Vec<u8>>,
) -> Result<Value, WordPressError> {
    let request = build_request(url, auth, body)?;
    let response = transport.send(&request).map_err(|reason| {
        WordPressError::unavailable(format!("WordPress site {url} is unavailable: {reason}"))
    })?;
    if response.status >= 400 {
        return Err(status_error(url, response.status, &response.body));
    }
    decode_json(&response.body, url)
}

/// Return the error for an HTTP status of 400 or above.
///
/// A 401 or 403 refuses the credentials, with the error document's
/// `code`/`message` appended when both are strings. Any other status reports
/// `WordPress`'s own error document, or an unavailable site when the body is
/// not one.
fn status_error(url: &str, status: u16, body: &[u8]) -> WordPressError {
    let detail = error_body(body);
    if matches!(status, 401 | 403) {
        return WordPressError::refused(url, status, detail);
    }
    match detail {
        Some((code, message)) => WordPressError::response(code, message, status),
        None => WordPressError::unavailable(format!(
            "WordPress site {url} returned HTTP {status} without a JSON error body"
        )),
    }
}

#[cfg(test)]
mod tests;
