// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the `minijinja` environment, its formatter and the page context.

use minijinja::{Environment, ErrorKind, context};

use super::{
    environment, format_value, poster_context, racejson, wordpress_page_context,
    wordpress_poster_context,
};
use crate::record::from_record;
use crate::sections::{poster_sections, sections_from_payload};
use crate::types::{RacePayload, SELF_TEST_NOTE, TRANSPARENT_PNG};

const PAYLOAD_SOLO: &str = include_str!("../../fixtures/payload-solo.json");

const FAKE_LOGO: &str = "data:image/png;base64,FAKE";

/// The poster plan as the template reads it: heading, then row count.
const POSTER_LOOP: &str =
    "{% for heading, rows in poster_sections %}{{ heading }}={{ rows | length }};{% endfor %}";

/// Read the solo fixture into a payload.
fn solo_payload() -> RacePayload {
    let record: serde_json::Value =
        serde_json::from_str(PAYLOAD_SOLO).expect("the fixture record parses");
    from_record(&record).expect("the fixture record reads")
}

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

// ------------------------------------------------- the WordPress context

#[test]
fn test_wordpress_page_context_given_a_plan_inlines_the_scoped_stylesheet() {
    // Arrange
    let payload = solo_payload();
    let plan = sections_from_payload(&payload);
    let context = wordpress_page_context(&payload, &plan, None);

    // Act
    let stylesheet = environment()
        .render_str("{{ compiled_css_wp | safe }}", &context)
        .expect("the inline template renders");

    // Assert
    assert!(stylesheet.starts_with("/* rivercrossing compiled_css_wp"));
}

#[test]
fn test_wordpress_page_context_given_a_plan_keeps_the_event_and_sections() {
    // Arrange
    let payload = solo_payload();
    let plan = sections_from_payload(&payload);
    let context = wordpress_page_context(&payload, &plan, None);

    // Act
    let rendered = environment()
        .render_str(
            "{{ event.title | safe }}|{{ sections.top_solo | length }}|{{ logo_src }}",
            &context,
        )
        .expect("the inline template renders");

    // Assert
    assert_eq!(
        rendered,
        format!(
            "{}|{}|{TRANSPARENT_PNG}",
            payload.event.title,
            plan.top_solo.len()
        )
    );
}

#[test]
fn test_wordpress_page_context_given_no_record_key_raises_the_undefined_error() {
    // Arrange
    let payload = solo_payload();
    let plan = sections_from_payload(&payload);
    let context = wordpress_page_context(&payload, &plan, None);

    // Act
    let result = environment().render_str("{{ payload }}", &context);

    // Assert
    let error = result.expect_err("the fragment context carries no record");
    assert_eq!(error.kind(), ErrorKind::UndefinedError);
}

#[test]
fn test_wordpress_page_context_given_no_dev_key_raises_the_undefined_error() {
    // Arrange
    let payload = solo_payload();
    let plan = sections_from_payload(&payload);
    let context = wordpress_page_context(&payload, &plan, None);

    // Act
    let result = environment().render_str("{{ dev }}", &context);

    // Assert
    let error = result.expect_err("the fragment context carries no dev flag");
    assert_eq!(error.kind(), ErrorKind::UndefinedError);
}

#[test]
fn test_wordpress_page_context_given_a_tie_note_renders_it() {
    // Arrange
    let mut payload = solo_payload();
    payload.tie_note = Some("held identical hands".to_owned());
    let plan = sections_from_payload(&payload);
    let context = wordpress_page_context(&payload, &plan, None);

    // Act
    let rendered = environment()
        .render_str("{{ tie_note }}", &context)
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, "held identical hands");
}

// -------------------------------------------------------- the poster context

#[test]
fn test_poster_context_given_a_payload_renders_the_heading_row_pairs() {
    // Arrange
    let payload = solo_payload();
    let context = poster_context(&payload, None);
    let expected = poster_sections(&payload)
        .iter()
        .map(|(heading, rows)| format!("{heading}={};", rows.len()))
        .collect::<Vec<String>>()
        .concat();

    // Act
    let rendered = environment()
        .render_str(POSTER_LOOP, &context)
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, expected);
}

#[test]
fn test_poster_context_given_no_logo_falls_back_to_the_transparent_png() {
    // Arrange
    let payload = solo_payload();
    let context = poster_context(&payload, None);

    // Act
    let rendered = environment()
        .render_str("{{ logo_src }}", &context)
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, TRANSPARENT_PNG);
}

#[test]
fn test_poster_context_given_a_logo_uses_it() {
    // Arrange
    let payload = solo_payload();
    let context = poster_context(&payload, Some(FAKE_LOGO));

    // Act
    let rendered = environment()
        .render_str("{{ logo_src }}", &context)
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, FAKE_LOGO);
}

#[test]
fn test_poster_context_given_an_organizer_uses_it_as_the_logo_alt() {
    // Arrange
    let payload = solo_payload();
    let context = poster_context(&payload, None);

    // Act
    let rendered = environment()
        .render_str("{{ logo_alt }}", &context)
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, payload.event.organizer);
}

#[test]
fn test_poster_context_given_an_unverified_ride_carries_the_self_test_note() {
    // Arrange
    let mut payload = solo_payload();
    payload.self_test_note = Some(SELF_TEST_NOTE.to_owned());
    let context = poster_context(&payload, None);

    // Act
    let rendered = environment()
        .render_str(
            "{% if self_test_note %}{{ self_test_note }}{% endif %}",
            &context,
        )
        .expect("the inline template renders");

    // Assert
    assert_eq!(rendered, SELF_TEST_NOTE);
}

#[test]
fn test_poster_context_inlines_the_unscoped_stylesheet() {
    // Arrange
    let payload = solo_payload();
    let context = poster_context(&payload, None);

    // Act
    let stylesheet = environment()
        .render_str("{{ compiled_css | safe }}", &context)
        .expect("the inline template renders");

    // Assert
    assert!(stylesheet.starts_with("/* rivercrossing compiled_css —"));
}

#[test]
fn test_poster_context_given_no_sections_key_raises_the_undefined_error() {
    // Arrange
    let payload = solo_payload();
    let context = poster_context(&payload, None);

    // Act
    let result = environment().render_str("{{ sections }}", &context);

    // Assert
    let error = result.expect_err("the poster context carries no page sections");
    assert_eq!(error.kind(), ErrorKind::UndefinedError);
}

#[test]
fn test_poster_context_given_no_record_key_raises_the_undefined_error() {
    // Arrange
    let payload = solo_payload();
    let context = poster_context(&payload, None);

    // Act
    let result = environment().render_str("{{ payload }}", &context);

    // Assert
    let error = result.expect_err("the poster context carries no record");
    assert_eq!(error.kind(), ErrorKind::UndefinedError);
}

#[test]
fn test_poster_context_given_no_tie_note_key_raises_the_undefined_error() {
    // Arrange
    let payload = solo_payload();
    let context = poster_context(&payload, None);

    // Act
    let result = environment().render_str("{{ tie_note }}", &context);

    // Assert
    let error = result.expect_err("the poster carries no tie note");
    assert_eq!(error.kind(), ErrorKind::UndefinedError);
}

#[test]
fn test_poster_context_given_no_dev_key_raises_the_undefined_error() {
    // Arrange
    let payload = solo_payload();
    let context = poster_context(&payload, None);

    // Act
    let result = environment().render_str("{{ dev }}", &context);

    // Assert
    let error = result.expect_err("the poster context carries no dev flag");
    assert_eq!(error.kind(), ErrorKind::UndefinedError);
}

#[test]
fn test_wordpress_poster_context_inlines_the_scoped_stylesheet() {
    // Arrange
    let payload = solo_payload();
    let context = wordpress_poster_context(&payload, None);

    // Act
    let stylesheet = environment()
        .render_str("{{ compiled_css_wp | safe }}", &context)
        .expect("the inline template renders");

    // Assert
    assert!(stylesheet.starts_with("/* rivercrossing compiled_css_wp"));
}

#[test]
fn test_wordpress_poster_context_keeps_the_poster_sections() {
    // Arrange
    let payload = solo_payload();
    let environment = environment();

    // Act
    let poster = environment
        .render_str(POSTER_LOOP, poster_context(&payload, None))
        .expect("the poster context renders");
    let wordpress = environment
        .render_str(POSTER_LOOP, wordpress_poster_context(&payload, None))
        .expect("the fragment context renders");

    // Assert
    assert_eq!(wordpress, poster);
}
