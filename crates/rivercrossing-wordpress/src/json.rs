// SPDX-License-Identifier: GPL-3.0-only

//! The oracle's JSON shape validators (`wordpress.py:378-491`).
//!
//! Every failure names the site URL and uses the oracle's wording, so the
//! caller's log matches what the app has always shown.

// Only the unit tests drive these validators until the public `discover` and
// `pages` entry points land in later phases.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the public discover and pages entry points, which land in later phases, call these validators"
    )
)]

use serde_json::{Map, Value};

use crate::error::WordPressError;

/// Return the JSON document that `raw` holds.
///
/// # Errors
///
/// Return [`WordPressError::Unavailable`] when `raw` is not UTF-8 or not JSON.
pub(crate) fn decode_json(raw: &[u8], url: &str) -> Result<Value, WordPressError> {
    let text = std::str::from_utf8(raw).map_err(|error| not_json(url, &error))?;
    serde_json::from_str(text).map_err(|error| not_json(url, &error))
}

/// Return the object that `payload` holds.
///
/// # Errors
///
/// Return [`WordPressError::Unavailable`] when `payload` is not a JSON object.
pub(crate) fn as_object<'a>(
    payload: &'a Value,
    url: &str,
) -> Result<&'a Map<String, Value>, WordPressError> {
    match payload {
        Value::Object(mapping) => Ok(mapping),
        _ => Err(WordPressError::unavailable(format!(
            "WordPress site {url} did not return a JSON object"
        ))),
    }
}

/// Return the items that `payload` holds.
///
/// # Errors
///
/// Return [`WordPressError::Unavailable`] when `payload` is not a JSON array.
pub(crate) fn as_array<'a>(payload: &'a Value, url: &str) -> Result<&'a [Value], WordPressError> {
    match payload {
        Value::Array(items) => Ok(items),
        _ => Err(WordPressError::unavailable(format!(
            "WordPress site {url} did not return a JSON array"
        ))),
    }
}

/// Return the integer field `name` of `value`.
///
/// `value` is the containing document, so a page that is not a JSON object
/// fails with the same message as a missing field. A JSON boolean is rejected,
/// matching the Rust reading rather than Python's `bool`-is-`int` behaviour;
/// `WordPress` never sends a boolean id.
///
/// # Errors
///
/// Return [`WordPressError::Unavailable`] when `value` is no object, or the
/// field is absent or holds no integer.
pub(crate) fn integer_field(value: &Value, name: &str, url: &str) -> Result<i64, WordPressError> {
    value.get(name).and_then(Value::as_i64).ok_or_else(|| {
        WordPressError::unavailable(format!(
            "WordPress site {url} returned a response without an integer {name}"
        ))
    })
}

/// Return the string field `name` of `mapping`.
///
/// The error names the field in single quotes, as the oracle's `!r` does.
///
/// # Errors
///
/// Return [`WordPressError::Unavailable`] when the field is absent or holds no
/// string.
pub(crate) fn text_field(
    mapping: &Map<String, Value>,
    name: &str,
    url: &str,
) -> Result<String, WordPressError> {
    mapping
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            WordPressError::unavailable(format!(
                "WordPress site {url} returned a response without a '{name}' field"
            ))
        })
}

/// Return the string items of `value`, in order.
///
/// A missing value or any other shape is empty, not a failure.
pub(crate) fn string_items(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

/// Return the error document that `raw` holds, when it is one.
///
/// The document qualifies only when it is a JSON object whose `code` and
/// `message` are both strings; every other shape is `None`.
pub(crate) fn error_body(raw: &[u8]) -> Option<(String, String)> {
    let Value::Object(mapping) = serde_json::from_slice(raw).ok()? else {
        return None;
    };
    let code = mapping.get("code")?.as_str()?.to_owned();
    let message = mapping.get("message")?.as_str()?.to_owned();
    Some((code, message))
}

/// Return the error for a body that the site did not send as JSON.
fn not_json(url: &str, reason: &impl std::fmt::Display) -> WordPressError {
    WordPressError::unavailable(format!(
        "WordPress site {url} did not return JSON: {reason}"
    ))
}

#[cfg(test)]
mod tests;
