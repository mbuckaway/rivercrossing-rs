// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the Publish-to-WordPress dialog's decisions.
//!
//! The label-to-value maps, the prefill, the slug algorithm and the pre-flight
//! validation are pinned here. `slugify` is the app's one slug algorithm, so
//! the parity cases below pin exactly what the export filenames use.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{
    PUBLISH_KIND_CHOICES, PublishForm, STATUS_CHOICES, default_slug, default_title, kind_value,
    slugify, status_value, validate,
};

/// A complete, valid dialog: an https site, its `WordPress` user, and the
/// title/slug the dialog derives from a ride.
const BASE_URL: &str = "https://example.test";
const USERNAME: &str = "example-user";
const PASSWORD: &str = "example-value";
const TITLE: &str = "GORBA Test Ride #1 — Full Results";
const SLUG: &str = "gorba-test-ride-1-full-results";

/// The two form fields `validate` never inspects: the typed parent page and
/// the status choice. A blank parent means top level.
const PARENT: &str = "";

/// The status the dialog opens on.
const STATUS: &str = "draft";

/// Return the valid form.
fn valid_form() -> PublishForm {
    PublishForm {
        base_url: BASE_URL.to_owned(),
        username: USERNAME.to_owned(),
        password: PASSWORD.to_owned(),
        title: TITLE.to_owned(),
        slug: SLUG.to_owned(),
        parent: PARENT.to_owned(),
        status: STATUS.to_owned(),
        kind: "full".to_owned(),
    }
}

// ------------------------------------------ STATUS_CHOICES: the choice

#[test]
fn test_publish_wordpress_status_choices_carry_the_four_wordpress_statuses_in_order() {
    // Arrange
    let expected = [
        ("Draft", "draft"),
        ("Publish", "publish"),
        ("Pending", "pending"),
        ("Private", "private"),
    ];

    // Act
    let choices = STATUS_CHOICES;

    // Assert
    assert_eq!(choices, expected);
}

#[rstest]
#[case("Draft", "draft")]
#[case("Publish", "publish")]
#[case("Pending", "pending")]
#[case("Private", "private")]
fn test_status_value_maps_every_choice_label_to_its_value(
    #[case] label: &str,
    #[case] expected: &str,
) {
    // Arrange
    let label = label.to_owned();

    // Act
    let value = status_value(&label);

    // Assert
    assert_eq!(value, expected);
}

#[rstest]
#[case("Scheduled")]
#[case("Future")]
#[case("Trash")]
#[case("")]
#[case("   ")]
fn test_status_value_falls_back_to_draft_for_any_other_label(#[case] label: &str) {
    // Arrange
    let label = label.to_owned();

    // Act
    let value = status_value(&label);

    // Assert
    assert_eq!(value, "draft");
}

// ------------------------------------ PUBLISH_KIND_CHOICES: the choice

#[test]
fn test_publish_wordpress_kind_choices_carry_the_two_kinds_in_order() {
    // Arrange
    let expected = [("Full results", "full"), ("Podium results", "podium")];

    // Act
    let choices = PUBLISH_KIND_CHOICES;

    // Assert
    assert_eq!(choices, expected);
}

#[rstest]
#[case("Full results", "full")]
#[case("Podium results", "podium")]
fn test_kind_value_maps_every_choice_label_to_its_value(
    #[case] label: &str,
    #[case] expected: &str,
) {
    // Arrange
    let label = label.to_owned();

    // Act
    let value = kind_value(&label);

    // Assert
    assert_eq!(value, expected);
}

#[rstest]
#[case("Podium")]
#[case("Results")]
#[case("full")]
#[case("podium")]
#[case("")]
#[case("   ")]
fn test_kind_value_falls_back_to_full_for_any_other_label(#[case] label: &str) {
    // Arrange
    let label = label.to_owned();

    // Act
    let value = kind_value(&label);

    // Assert
    assert_eq!(value, "full");
}

#[rstest]
#[case("Full results")]
#[case("Podium results")]
#[case("bogus")]
#[case("")]
fn test_kind_value_given_any_text_returns_a_choice_value(#[case] label: &str) {
    // Arrange
    let label = label.to_owned();

    // Act
    let value = kind_value(&label);

    // Assert
    assert!(matches!(value, "full" | "podium"));
}

// ---------------------------- slugify: the same algorithm as the app

#[rstest]
#[case("GORBA Test Ride #1", "gorba-test-ride-1")]
#[case("", "results")]
#[case("!!!", "results")]
#[case("   ", "results")]
#[case("---", "results")]
#[case("Sea to Sky", "sea-to-sky")]
#[case("a--b___c", "a-b-c")]
#[case("  padded name  ", "padded-name")]
#[case("--leading-and-trailing--", "leading-and-trailing")]
#[case("5.5 km loop", "5-5-km-loop")]
#[case("2026", "2026")]
#[case("Étape—Ünïcode", "étape-ünïcode")]
#[case("Ünïcode", "ünïcode")]
fn test_slugify_applies_the_ride_slug_transform(#[case] name: &str, #[case] expected: &str) {
    // Arrange
    let name = name.to_owned();

    // Act
    let slug = slugify(&name);

    // Assert
    assert_eq!(slug, expected);
}

#[rstest]
#[case("GORBA Test Ride #1")]
#[case("")]
#[case("   ")]
#[case("Étape—Ünïcode")]
#[case("--a--b--")]
fn test_slugify_output_is_slug_shaped_and_idempotent(#[case] name: &str) {
    // Arrange
    let name = name.to_owned();

    // Act
    let slug = slugify(&name);

    // Assert
    assert_ne!(slug.as_str(), "");
    assert!(!slug.contains("--"));
    assert!(!slug.starts_with('-'));
    assert!(!slug.ends_with('-'));
    assert_eq!(slugify(&slug), slug);
}

// --------------------------- default_title / default_slug: the prefill

#[rstest]
#[case("full", "GORBA Test Ride #1 — Full Results")]
#[case("podium", "GORBA Test Ride #1 — Podium Results")]
#[case("Full results", "GORBA Test Ride #1 — Full Results")]
#[case("Podium results", "GORBA Test Ride #1 — Podium Results")]
fn test_default_title_given_each_kind_appends_its_suffix(
    #[case] kind: &str,
    #[case] expected: &str,
) {
    // Arrange
    let kind = kind.to_owned();

    // Act
    let title = default_title("GORBA Test Ride #1", &kind);

    // Assert
    assert_eq!(title, expected);
}

#[rstest]
#[case("")]
#[case("   ")]
#[case("bogus")]
#[case("podii")]
fn test_default_title_given_an_unknown_kind_appends_the_full_results_suffix(#[case] kind: &str) {
    // Arrange
    let kind = kind.to_owned();

    // Act
    let title = default_title("GORBA Test Ride #1", &kind);

    // Assert
    assert_eq!(title, "GORBA Test Ride #1 — Full Results");
}

#[rstest]
#[case("full", "gorba-test-ride-1-full-results")]
#[case("podium", "gorba-test-ride-1-podium-results")]
#[case("Full results", "gorba-test-ride-1-full-results")]
#[case("Podium results", "gorba-test-ride-1-podium-results")]
fn test_default_slug_given_each_kind_slugifies_the_kind_tail(
    #[case] kind: &str,
    #[case] expected: &str,
) {
    // Arrange
    let kind = kind.to_owned();

    // Act
    let slug = default_slug("GORBA Test Ride #1", &kind);

    // Assert
    assert_eq!(slug, expected);
}

#[rstest]
#[case("")]
#[case("bogus")]
fn test_default_slug_given_an_unknown_kind_slugifies_the_full_results_tail(#[case] kind: &str) {
    // Arrange
    let kind = kind.to_owned();

    // Act
    let slug = default_slug("GORBA Test Ride #1", &kind);

    // Assert
    assert_eq!(slug, "gorba-test-ride-1-full-results");
}

#[test]
fn test_default_slug_of_a_nameless_ride_is_full_results() {
    // Arrange
    let name = "";
    let kind = "full";

    // Act
    let slug = default_slug(name, kind);

    // Assert
    assert_eq!(slug, "full-results");
}

#[test]
fn test_default_slug_of_a_nameless_podium_ride_is_podium_results() {
    // Arrange
    let name = "";
    let kind = "podium";

    // Act
    let slug = default_slug(name, kind);

    // Assert
    assert_eq!(slug, "podium-results");
}

// ------------------------------------------------- validate: the rules

#[test]
fn test_validate_accepts_a_complete_https_form() {
    // Arrange
    let form = valid_form();

    // Act
    let errors = validate(&form);

    // Assert
    assert_eq!(errors, Vec::<String>::new());
}

#[rstest]
#[case(0, "site URL is required")]
#[case(1, "username is required")]
#[case(2, "application password is required")]
#[case(3, "page title is required")]
#[case(4, "slug is required")]
fn test_validate_reports_the_one_blank_field(#[case] field: usize, #[case] expected: &str) {
    // Arrange
    let mut form = valid_form();
    clear_field(&mut form, field);

    // Act
    let errors = validate(&form);

    // Assert
    assert_eq!(errors, vec![expected.to_owned()]);
}

/// Blank the form field that *field* numbers, in the form's own order.
fn clear_field(form: &mut PublishForm, field: usize) {
    let slot = match field {
        0 => &mut form.base_url,
        1 => &mut form.username,
        2 => &mut form.password,
        3 => &mut form.title,
        _ => &mut form.slug,
    };
    slot.clear();
}

#[rstest]
#[case("http://example.test")]
#[case("example.test")]
#[case("//example.test")]
#[case("ftp://example.test")]
fn test_validate_rejects_a_site_url_that_is_not_https(#[case] base_url: &str) {
    // Arrange
    let mut form = valid_form();
    form.base_url = base_url.to_owned();

    // Act
    let errors = validate(&form);

    // Assert
    assert_eq!(
        errors,
        vec![
            "site URL must start with https:// (WordPress credentials are only sent over HTTPS)"
                .to_owned()
        ]
    );
}

#[test]
fn test_validate_accepts_an_https_url_with_a_path() {
    // Arrange
    let mut form = valid_form();
    form.base_url = "https://example.test/blog".to_owned();

    // Act
    let errors = validate(&form);

    // Assert
    assert_eq!(errors, Vec::<String>::new());
}

#[test]
fn test_validate_reports_every_blank_field_top_of_form_first() {
    // Arrange
    let mut form = valid_form();
    form.base_url = String::new();
    form.username = String::new();
    form.password = String::new();
    form.title = String::new();
    form.slug = String::new();

    // Act
    let errors = validate(&form);

    // Assert
    assert_eq!(
        errors,
        vec![
            "site URL is required".to_owned(),
            "username is required".to_owned(),
            "application password is required".to_owned(),
            "page title is required".to_owned(),
            "slug is required".to_owned(),
        ]
    );
}

#[test]
fn test_validate_keeps_its_field_order_when_the_url_is_http() {
    // Arrange
    let mut form = valid_form();
    form.base_url = "http://example.test".to_owned();
    form.username = String::new();
    form.password = String::new();
    form.title = String::new();
    form.slug = String::new();

    // Act
    let errors = validate(&form);

    // Assert
    assert_eq!(
        errors,
        vec![
            "site URL must start with https:// (WordPress credentials are only sent over HTTPS)"
                .to_owned(),
            "username is required".to_owned(),
            "application password is required".to_owned(),
            "page title is required".to_owned(),
            "slug is required".to_owned(),
        ]
    );
}

// ------------------------------------ PublishForm: the dialog's form

#[test]
fn test_publish_form_is_valid_when_every_field_passes() {
    // Arrange
    let form = valid_form();

    // Act
    let valid = form.is_valid();

    // Assert
    assert!(valid);
}

#[test]
fn test_publish_form_is_invalid_when_a_field_is_blank() {
    // Arrange
    let form = PublishForm {
        username: String::new(),
        ..valid_form()
    };

    // Act
    let valid = form.is_valid();

    // Assert
    assert!(!valid);
}

#[test]
fn test_publish_form_errors_are_validates_own_messages() {
    // Arrange
    let form = PublishForm {
        username: String::new(),
        slug: String::new(),
        ..valid_form()
    };

    // Act
    let errors = form.errors();

    // Assert
    assert_eq!(
        errors,
        vec![
            "username is required".to_owned(),
            "slug is required".to_owned()
        ]
    );
}

#[rstest]
#[case("")]
#[case("12")]
#[case("results")]
#[case("grand-parent")]
fn test_publish_form_accepts_any_parent_page_text(#[case] parent: &str) {
    // Arrange
    let form = PublishForm {
        parent: parent.to_owned(),
        ..valid_form()
    };

    // Act
    let valid = form.is_valid();

    // Assert
    assert!(valid);
}

#[rstest]
#[case("full")]
#[case("podium")]
fn test_publish_form_accepts_either_publish_kind(#[case] kind: &str) {
    // Arrange
    let form = PublishForm {
        kind: kind.to_owned(),
        ..valid_form()
    };

    // Act
    let valid = form.is_valid();

    // Assert
    assert!(valid);
}

#[test]
fn test_publish_form_defaults_its_kind_to_full() {
    // Arrange
    let form = PublishForm::default();

    // Act
    let kind = form.kind;

    // Assert
    assert_eq!(kind, "full");
}

#[test]
fn test_publish_form_given_a_podium_kind_keeps_it() {
    // Arrange
    let form = PublishForm {
        kind: "podium".to_owned(),
        ..valid_form()
    };

    // Act
    let kind = form.kind;

    // Assert
    assert_eq!(kind, "podium");
}
