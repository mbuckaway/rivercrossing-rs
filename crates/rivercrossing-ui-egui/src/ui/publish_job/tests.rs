// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the publish job: the checkpoints, the create/update split and the failures.

use rivercrossing_htmlexport::types::{ExportInput, ExportOptions, RideInfo};
use rivercrossing_wordpress::PublishedPage;
use time::Month;

use super::{MockPageWriter, PublishOutcome, run_publish};
use crate::ui::presenters::publish_wordpress::PublishForm;
use crate::ui::state::exports::PublishStart;

/// Return the job's page payload.
fn payload() -> Result<ExportInput, Box<dyn std::error::Error>> {
    let date = time::Date::from_calendar_date(2026, Month::September, 20)?;
    Ok(ExportInput {
        ride: RideInfo {
            name: "GORBA EPIC 2026".to_owned(),
            event_date: date,
            venue: "Sea to Sky Gondola".to_owned(),
            lap_km: 8.0,
            organizer: "GORBA".to_owned(),
            scorer: "K. Singh".to_owned(),
        },
        placed: Vec::new(),
        options: ExportOptions::default(),
        generated: "Generated 2026-09-20".to_owned(),
        logo_src: None,
        team_logos: std::collections::BTreeMap::new(),
        riders: 0,
        self_test_unverified: false,
        tie_note: None,
    })
}

/// Return one valid publish start with *kind*.
fn start(kind: &str) -> Result<PublishStart, Box<dyn std::error::Error>> {
    Ok(PublishStart {
        form: PublishForm {
            base_url: "https://example.test".to_owned(),
            username: "organizer".to_owned(),
            password: "abcd efgh".to_owned(),
            title: "GORBA EPIC 2026 — Results".to_owned(),
            slug: "gorba-epic-2026-results".to_owned(),
            parent: "0".to_owned(),
            status: "publish".to_owned(),
            kind: kind.to_owned(),
        },
        input: Box::new(payload()?),
    })
}

/// Return the page a publish reports.
fn page(created: bool) -> PublishedPage {
    PublishedPage {
        id: 7,
        link: "https://example.test/?page_id=7".to_owned(),
        status: "publish".to_owned(),
        created,
    }
}

#[test]
fn test_run_publish_given_no_existing_page_creates_it() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut writer = MockPageWriter::new();
    writer
        .expect_find()
        .with(
            mockall::predicate::eq("https://example.test"),
            mockall::predicate::always(),
            mockall::predicate::eq("gorba-epic-2026-results"),
        )
        .times(1)
        .returning(|_, _, _| Ok(None));
    writer
        .expect_publish()
        .withf(|base, _auth, draft| {
            base == "https://example.test" && draft.page_id.is_none() && draft.parent == 0
        })
        .times(1)
        .returning(|_, _, _| Ok(page(true)));

    // Act
    let outcome = run_publish(&writer, &start("full")?, &|| false)?;

    // Assert
    assert_eq!(outcome, PublishOutcome::Published(page(true)));
    Ok(())
}

#[test]
fn test_run_publish_given_an_existing_page_updates_it() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut writer = MockPageWriter::new();
    writer
        .expect_find()
        .times(1)
        .returning(|_, _, _| Ok(Some(7)));
    writer
        .expect_publish()
        .withf(|_, _, draft| draft.page_id == Some(7))
        .times(1)
        .returning(|_, _, _| Ok(page(false)));

    // Act
    let outcome = run_publish(&writer, &start("podium")?, &|| false)?;

    // Assert
    assert_eq!(outcome, PublishOutcome::Published(page(false)));
    Ok(())
}

#[test]
fn test_run_publish_given_a_cancel_before_the_lookup_contacts_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut writer = MockPageWriter::new();
    writer.expect_find().times(0);
    writer.expect_publish().times(0);

    // Act
    let outcome = run_publish(&writer, &start("full")?, &|| true)?;

    // Assert
    assert_eq!(outcome, PublishOutcome::Cancelled);
    Ok(())
}

#[test]
fn test_run_publish_given_a_cancel_after_the_lookup_does_not_publish()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut writer = MockPageWriter::new();
    writer.expect_find().times(1).returning(|_, _, _| Ok(None));
    writer.expect_publish().times(0);
    let polls = std::cell::Cell::new(0_u32);
    let cancel = || {
        let seen = polls.get();
        polls.set(seen + 1);
        seen >= 1
    };

    // Act
    let outcome = run_publish(&writer, &start("full")?, &cancel)?;

    // Assert
    assert_eq!(outcome, PublishOutcome::Cancelled);
    Ok(())
}

#[test]
fn test_run_publish_given_a_failed_lookup_returns_its_sentence()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut writer = MockPageWriter::new();
    writer
        .expect_find()
        .times(1)
        .returning(|_, _, _| Err("the site refused the login".to_owned()));

    // Act
    let result = run_publish(&writer, &start("full")?, &|| false);

    // Assert
    assert_eq!(result, Err("the site refused the login".to_owned()));
    Ok(())
}

#[test]
fn test_run_publish_given_a_failed_publish_returns_its_sentence()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut writer = MockPageWriter::new();
    writer.expect_find().times(1).returning(|_, _, _| Ok(None));
    writer
        .expect_publish()
        .times(1)
        .returning(|_, _, _| Err("the site timed out".to_owned()));

    // Act
    let result = run_publish(&writer, &start("full")?, &|| false);

    // Assert
    assert_eq!(result, Err("the site timed out".to_owned()));
    Ok(())
}

/// Return one valid publish start whose parent field is *parent*.
fn start_with_parent(parent: &str) -> Result<PublishStart, Box<dyn std::error::Error>> {
    let mut start = start("full")?;
    parent.clone_into(&mut start.form.parent);
    Ok(start)
}

#[test]
fn test_run_publish_given_a_numeric_parent_uses_it_as_the_page_id()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut writer = MockPageWriter::new();
    writer
        .expect_find()
        .with(
            mockall::predicate::eq("https://example.test"),
            mockall::predicate::always(),
            mockall::predicate::eq("gorba-epic-2026-results"),
        )
        .times(1)
        .returning(|_, _, _| Ok(None));
    writer
        .expect_publish()
        .withf(|_, _, draft| draft.page_id.is_none() && draft.parent == 42)
        .times(1)
        .returning(|_, _, _| Ok(page(true)));

    // Act
    let outcome = run_publish(&writer, &start_with_parent(" 42 ")?, &|| false)?;

    // Assert
    assert_eq!(outcome, PublishOutcome::Published(page(true)));
    Ok(())
}

#[test]
fn test_run_publish_given_a_blank_parent_publishes_at_the_top_level()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut writer = MockPageWriter::new();
    writer.expect_find().times(1).returning(|_, _, _| Ok(None));
    writer
        .expect_publish()
        .withf(|_, _, draft| draft.parent == 0)
        .times(1)
        .returning(|_, _, _| Ok(page(true)));

    // Act
    let outcome = run_publish(&writer, &start_with_parent("   ")?, &|| false)?;

    // Assert
    assert_eq!(outcome, PublishOutcome::Published(page(true)));
    Ok(())
}

#[test]
fn test_run_publish_given_a_slug_parent_resolves_the_page_id()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut writer = MockPageWriter::new();
    writer
        .expect_find()
        .with(
            mockall::predicate::eq("https://example.test"),
            mockall::predicate::always(),
            mockall::predicate::eq("gorba-epic-2026-results"),
        )
        .times(1)
        .returning(|_, _, _| Ok(None));
    writer
        .expect_find()
        .with(
            mockall::predicate::eq("https://example.test"),
            mockall::predicate::always(),
            mockall::predicate::eq("results-archive"),
        )
        .times(1)
        .returning(|_, _, _| Ok(Some(9)));
    writer
        .expect_publish()
        .withf(|_, _, draft| draft.parent == 9 && draft.page_id.is_none())
        .times(1)
        .returning(|_, _, _| Ok(page(true)));

    // Act
    let outcome = run_publish(&writer, &start_with_parent("results-archive")?, &|| false)?;

    // Assert
    assert_eq!(outcome, PublishOutcome::Published(page(true)));
    Ok(())
}

#[test]
fn test_run_publish_given_a_missing_slug_parent_publishes_at_the_top_level()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut writer = MockPageWriter::new();
    writer
        .expect_find()
        .with(
            mockall::predicate::always(),
            mockall::predicate::always(),
            mockall::predicate::eq("gorba-epic-2026-results"),
        )
        .times(1)
        .returning(|_, _, _| Ok(None));
    writer
        .expect_find()
        .with(
            mockall::predicate::always(),
            mockall::predicate::always(),
            mockall::predicate::eq("no-such-page"),
        )
        .times(1)
        .returning(|_, _, _| Ok(None));
    writer
        .expect_publish()
        .withf(|_, _, draft| draft.parent == 0)
        .times(1)
        .returning(|_, _, _| Ok(page(true)));

    // Act
    let outcome = run_publish(&writer, &start_with_parent("no-such-page")?, &|| false)?;

    // Assert
    assert_eq!(outcome, PublishOutcome::Published(page(true)));
    Ok(())
}

#[test]
fn test_run_publish_given_a_failed_parent_lookup_returns_its_sentence()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut writer = MockPageWriter::new();
    writer
        .expect_find()
        .with(
            mockall::predicate::always(),
            mockall::predicate::always(),
            mockall::predicate::eq("gorba-epic-2026-results"),
        )
        .times(1)
        .returning(|_, _, _| Ok(None));
    writer
        .expect_find()
        .with(
            mockall::predicate::always(),
            mockall::predicate::always(),
            mockall::predicate::eq("broken-parent"),
        )
        .times(1)
        .returning(|_, _, _| Err("the site refused the parent lookup".to_owned()));
    writer.expect_publish().times(0);

    // Act
    let result = run_publish(&writer, &start_with_parent("broken-parent")?, &|| false);

    // Assert
    assert_eq!(result, Err("the site refused the parent lookup".to_owned()));
    Ok(())
}
