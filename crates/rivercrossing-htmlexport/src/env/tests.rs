// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the `minijinja` environment, its formatter and the page context.

use minijinja::{Environment, ErrorKind, context};

use super::{environment, format_value, racejson};

#[test]
fn test_format_value_given_every_markupsafe_character_writes_its_entity() {
    // Arrange
    let env = environment();

    // Act
    let rendered = env
        .render_str("{{ v }}", context! { v => "&<>\"'/" })
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, "&amp;&lt;&gt;&#34;&#39;/");
}

#[test]
fn test_format_value_given_a_safe_string_writes_it_verbatim() {
    // Arrange
    let env = environment();

    // Act
    let rendered = env
        .render_str("{{ v | safe }}", context! { v => "&<>\"'/" })
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, "&<>\"'/");
}

#[test]
fn test_format_value_given_an_integral_float_writes_an_integer() {
    // Arrange
    let env = environment();

    // Act
    let rendered = env
        .render_str("{{ km }}", context! { km => 8.0_f64 })
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, "8");
}

#[test]
fn test_format_value_given_a_fractional_float_keeps_the_fraction() {
    // Arrange
    let env = environment();

    // Act
    let rendered = env
        .render_str("{{ km }}", context! { km => 6.5_f64 })
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, "6.5");
}

#[test]
fn test_format_value_given_a_boolean_writes_the_python_spelling() {
    // Arrange
    let env = environment();

    // Act
    let rendered = env
        .render_str("{{ v }}", context! { v => true })
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, "True");
}

#[test]
fn test_format_value_given_no_auto_escape_writes_the_string_verbatim() {
    // Arrange
    let mut env = Environment::new();
    env.set_formatter(format_value);

    // Act
    let rendered = env
        .render_str("{{ v }}", context! { v => "&<" })
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, "&<");
}

#[test]
fn test_environment_given_a_missing_attribute_raises_the_undefined_error() {
    // Arrange
    let env = environment();

    // Act
    let result = env.render_str("{{ event.title }}", context! {});

    // Assert
    let error = result.expect_err("StrictUndefined refuses a missing attribute");
    assert_eq!(error.kind(), ErrorKind::UndefinedError);
}

#[test]
fn test_racejson_given_a_closing_tag_escapes_its_slash() {
    // Act
    let escaped = racejson("a</script>b");

    // Assert
    assert_eq!(escaped, "a<\\/script>b");
}
