// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the oracle's JSON shape validators.

use pretty_assertions::assert_eq;
use serde_json::{Map, Value, json};

use super::{
    as_array, as_object, decode_json, error_body, integer_field, string_items, text_field,
};

const URL: &str = "https://x";

#[test]
fn test_decode_json_given_valid_json_returns_the_value() {
    // Arrange
    let raw = br#"{"namespaces": ["wp/v2"], "count": 2}"#;

    // Act
    let result = decode_json(raw, URL);

    // Assert
    assert_eq!(result, Ok(json!({"namespaces": ["wp/v2"], "count": 2})));
}

#[test]
fn test_decode_json_given_invalid_utf8_returns_unavailable() {
    // Arrange
    let raw = [0x7b, 0xff, 0x7d];

    // Act
    let result = decode_json(&raw, URL).map_err(|error| error.to_string());

    // Assert
    assert!(matches!(
        result,
        Err(ref text) if text.starts_with("WordPress site https://x did not return JSON: ")
    ));
}

#[test]
fn test_decode_json_given_truncated_json_returns_unavailable() {
    // Arrange
    let raw = br#"{"namespaces": ["#;

    // Act
    let result = decode_json(raw, URL).map_err(|error| error.to_string());

    // Assert
    assert!(matches!(
        result,
        Err(ref text) if text.starts_with("WordPress site https://x did not return JSON: ")
    ));
}

#[test]
fn test_as_object_and_as_array_accept_their_own_shape() {
    // Arrange
    let object_payload = json!({"a": 1});
    let array_payload = json!([1, 2]);

    // Act
    let object = as_object(object_payload, URL);
    let array = as_array(array_payload, URL);

    // Assert
    assert_eq!(
        object,
        Ok(Map::from_iter([("a".to_owned(), Value::from(1))]))
    );
    assert_eq!(array, Ok(vec![Value::from(1), Value::from(2)]));
}

#[test]
fn test_as_object_and_as_array_reject_the_other_shape() {
    // Arrange
    let array_payload = json!([1, 2]);
    let object_payload = json!({"a": 1});

    // Act
    let object = as_object(array_payload, URL).map_err(|error| error.to_string());
    let array = as_array(object_payload, URL).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        object,
        Err("WordPress site https://x did not return a JSON object".to_owned())
    );
    assert_eq!(
        array,
        Err("WordPress site https://x did not return a JSON array".to_owned())
    );
}

#[test]
fn test_integer_field_returns_the_integer() {
    // Arrange
    let mapping = Map::from_iter([("id".to_owned(), Value::from(42))]);

    // Act
    let result = integer_field(&mapping, "id", URL);

    // Assert
    assert_eq!(result, Ok(42));
}

#[test]
fn test_integer_field_given_missing_string_or_boolean() {
    // Arrange
    let mapping = Map::from_iter([
        ("text".to_owned(), Value::from("7")),
        ("flag".to_owned(), Value::Bool(true)),
    ]);

    // Act
    let missing = integer_field(&mapping, "id", URL).map_err(|error| error.to_string());
    let text = integer_field(&mapping, "text", URL).map_err(|error| error.to_string());
    let flag = integer_field(&mapping, "flag", URL).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        [missing, text, flag],
        [
            Err("WordPress site https://x returned a response without an integer id".to_owned()),
            Err("WordPress site https://x returned a response without an integer text".to_owned()),
            Err("WordPress site https://x returned a response without an integer flag".to_owned()),
        ]
    );
}

#[test]
fn test_text_field_returns_the_text() {
    // Arrange
    let mapping = Map::from_iter([("link".to_owned(), Value::from("https://x/page"))]);

    // Act
    let result = text_field(&mapping, "link", URL);

    // Assert
    assert_eq!(result, Ok("https://x/page".to_owned()));
}

#[test]
fn test_text_field_names_the_field_in_single_quotes() {
    // Arrange
    let missing = Map::new();
    let numeric = Map::from_iter([("link".to_owned(), Value::from(7))]);

    // Act
    let absent = text_field(&missing, "link", URL).map_err(|error| error.to_string());
    let wrong_shape = text_field(&numeric, "link", URL).map_err(|error| error.to_string());

    // Assert
    assert_eq!(
        [absent, wrong_shape],
        [
            Err("WordPress site https://x returned a response without a 'link' field".to_owned()),
            Err("WordPress site https://x returned a response without a 'link' field".to_owned()),
        ]
    );
}

#[test]
fn test_string_items_given_absent_empty_and_mixed() {
    // Arrange
    let empty_array = json!([]);
    let mixed_array = json!(["wp/v2", 1, "wp/v2/posts", null, true]);
    let scalar = json!("wp/v2");

    // Act
    let absent = string_items(None);
    let empty = string_items(Some(&empty_array));
    let mixed = string_items(Some(&mixed_array));
    let not_an_array = string_items(Some(&scalar));

    // Assert
    assert_eq!(
        [absent, empty, mixed, not_an_array],
        [
            vec![],
            vec![],
            vec!["wp/v2".to_owned(), "wp/v2/posts".to_owned()],
            vec![],
        ]
    );
}

#[test]
fn test_error_body_given_malformed_shapes() {
    // Arrange
    let array_body = br#"["code", "message"]"#;
    let integer_code = br#"{"code": 1, "message": "m"}"#;
    let integer_message = br#"{"code": "c", "message": 2}"#;
    let missing_message = br#"{"code": "c"}"#;
    let not_json = b"not json";
    let invalid_utf8 = [0xff, 0xfe];

    // Act
    let bodies = [
        error_body(array_body),
        error_body(integer_code),
        error_body(integer_message),
        error_body(missing_message),
        error_body(not_json),
        error_body(&invalid_utf8),
    ];

    // Assert
    assert_eq!(bodies, [None, None, None, None, None, None]);
}

#[test]
fn test_error_body_given_a_complete_document() {
    // Arrange
    let raw = br#"{"code": "rest_forbidden", "message": "Sorry"}"#;

    // Act
    let body = error_body(raw);

    // Assert
    assert_eq!(
        body,
        Some(("rest_forbidden".to_owned(), "Sorry".to_owned()))
    );
}
