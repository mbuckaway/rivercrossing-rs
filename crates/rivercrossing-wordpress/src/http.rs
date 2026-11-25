// SPDX-License-Identifier: GPL-3.0-only

//! The crate-private HTTP seam and the URL helpers that every request uses.

use url::form_urlencoded::Serializer;

use crate::auth::BasicAuth;
use crate::error::WordPressError;

/// The HTTP method that the crate sends.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the discover, pages and transport modules, which land in later phases, use the seam"
    )
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Method {
    /// A read with no request body.
    Get,
    /// A write with a JSON body.
    Post,
}

/// One request handed to the [`Transport`] seam.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the discover, pages and transport modules, which land in later phases, use the seam"
    )
)]
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
#[expect(
    dead_code,
    reason = "the transport module, which lands in a later phase, returns this type"
)]
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
#[expect(
    dead_code,
    reason = "the discover, pages and transport modules, which land in later phases, use the trait"
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the discover, pages and transport modules, which land in later phases, use this helper"
    )
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the discover, pages and transport modules, which land in later phases, call this gate"
    )
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the discover and pages modules, which land in later phases, build URLs with this helper"
    )
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the discover and pages modules, which land in later phases, build requests with this helper"
    )
)]
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

#[cfg(test)]
mod tests;
