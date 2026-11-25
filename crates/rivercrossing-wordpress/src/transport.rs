// SPDX-License-Identifier: GPL-3.0-only

//! The production transport: a blocking `ureq` agent with the pinned policy.

use std::time::Duration;

use crate::http::{HttpRequest, HttpResponse, Method, Transport};

/// The response cap: larger than any `WordPress` page, and 64 times the
/// `ureq` default of 10 MiB.
pub(crate) const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;

/// The operation budget in seconds: one request may take this long, its
/// redirects and its body read included.
pub(crate) const TIMEOUT_SECONDS: u64 = 30;

/// Return the agent policy for every `WordPress` request.
///
/// A status is a value, so a `WordPress` error body survives a 4xx or 5xx
/// answer; a redirect carries no `Authorization` header; and one operation is
/// bounded at 30 s.
pub(crate) fn agent_config() -> ureq::config::Config {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(TIMEOUT_SECONDS)))
        .build()
}

/// The production transport, backed by one [`ureq::Agent`].
pub(crate) struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    /// Return a transport whose agent runs the pinned policy.
    pub(crate) fn new() -> Self {
        Self {
            agent: ureq::Agent::new_with_config(agent_config()),
        }
    }
}

impl std::fmt::Debug for UreqTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UreqTransport")
            .finish_non_exhaustive()
    }
}

impl Transport for UreqTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, String> {
        let answer = match request.method {
            Method::Get => {
                with_headers(self.agent.get(request.url.as_str()), &request.headers).call()
            }
            Method::Post => {
                let payload = request.body.as_deref().unwrap_or_default();
                with_headers(self.agent.post(request.url.as_str()), &request.headers).send(payload)
            }
        };
        let mut response = answer.map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .map_err(|error| error.to_string())?;
        Ok(HttpResponse { status, body })
    }
}

/// Return `builder` with every recorded header attached, in order.
fn with_headers<B>(
    mut builder: ureq::RequestBuilder<B>,
    headers: &[(String, String)],
) -> ureq::RequestBuilder<B> {
    for (name, value) in headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    builder
}

#[cfg(test)]
mod tests;
