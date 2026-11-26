// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the audit trail dialog's rows and its two filters.

use std::collections::BTreeSet;

use pretty_assertions::assert_eq;
use rivercrossing_ride::Timestamp;
use rivercrossing_roster::enums::{EntryMode, PlateModel};
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::Rider;
use rivercrossing_store::AuditRow;
use time::{Date, Month, OffsetDateTime, Time};

use super::{ACTION_CHOICES, ALL_ACTIONS, AuditRowModel, filter, newest_first, rows};

/// The date every fixture instant uses.
const EVENT_DATE: (i32, Month, u8) = (2026, Month::September, 20);

// ------------------------------------------------------------- fixtures

/// Return the fixture instant at *hour*:*minute*:*second* on the event date.
fn instant(hour: u8, minute: u8, second: u8) -> Result<Timestamp, String> {
    let (year, month, day) = EVENT_DATE;
    let date = Date::from_calendar_date(year, month, day).map_err(|error| error.to_string())?;
    let time = Time::from_hms(hour, minute, second).map_err(|error| error.to_string())?;
    Ok(OffsetDateTime::new_utc(date, time))
}

/// Build one store audit row.
#[expect(
    clippy::too_many_arguments,
    reason = "one fixture call spells the row's instant, action, entry cell and reason"
)]
fn audit_row(hour: u8, action: &str, entry: &str, reason: &str) -> Result<AuditRow, String> {
    Ok(AuditRow {
        at: instant(hour, 0, 0)?,
        action: action.to_owned(),
        entry: entry.to_owned(),
        reason: reason.to_owned(),
    })
}

/// Return a MIXED rider-pooled roster: one solo entry and one pooled team.
fn fixture_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(7))
        .map_err(|error| error.to_string())?;
    roster
        .create_solo_entry(
            Rider {
                first_name: "J.".to_owned(),
                last_name: "Okafor".to_owned(),
                ..Rider::default()
            },
            "45",
        )
        .map_err(|error| error.to_string())?;
    roster
        .create_team_entry(TeamFields {
            display_name: "Trail Blazers",
            riders: vec![
                Rider {
                    first_name: "A.".to_owned(),
                    last_name: "Roy".to_owned(),
                    plate: Some("77".to_owned()),
                    ..Rider::default()
                },
                Rider {
                    first_name: "K.".to_owned(),
                    last_name: "Singh".to_owned(),
                    plate: Some("78".to_owned()),
                    ..Rider::default()
                },
            ],
            ..TeamFields::default()
        })
        .map_err(|error| error.to_string())?;
    Ok(roster)
}

/// Return rows that span several actions and both roster entries.
fn mixed_rows() -> Result<Vec<AuditRow>, String> {
    Ok(vec![
        audit_row(14, "dnf", "45", "mechanical failure")?,
        audit_row(13, "shoe_reshuffle", "", "0 jokers added")?,
        audit_row(12, "deal_manual", "45", "flag confirmed")?,
        audit_row(11, "record_crossing", "45", "J. Okafor · solo")?,
        audit_row(10, "edit_crossing", "77", "mis-keyed time")?,
        audit_row(9, "start", "", "0:00:00")?,
    ])
}

// ------------------------------------------------------------ newest first

#[test]
fn test_audit_newest_first_given_shuffled_rows_sorts_by_instant_descending() -> Result<(), String> {
    // Arrange
    let mut store_rows = vec![
        audit_row(10, "start", "", "")?,
        audit_row(14, "dnf", "45", "")?,
        audit_row(12, "deal_manual", "45", "")?,
    ];

    // Act
    newest_first(&mut store_rows);

    // Assert
    assert_eq!(
        store_rows
            .iter()
            .map(|row| row.at.hour())
            .collect::<Vec<u8>>(),
        vec![14, 12, 10]
    );
    Ok(())
}

#[test]
fn test_audit_newest_first_given_equal_instants_keeps_the_input_order() -> Result<(), String> {
    // Arrange
    let mut store_rows = vec![
        audit_row(10, "start", "", "")?,
        audit_row(10, "record_crossing", "45", "")?,
    ];

    // Act
    newest_first(&mut store_rows);

    // Assert
    assert_eq!(
        store_rows
            .iter()
            .map(|row| row.action.as_str())
            .collect::<Vec<&str>>(),
        vec!["start", "record_crossing"]
    );
    Ok(())
}

#[test]
fn test_audit_newest_first_given_an_empty_list_stays_empty() {
    // Arrange
    let mut store_rows: Vec<AuditRow> = Vec::new();

    // Act
    newest_first(&mut store_rows);

    // Assert
    assert_eq!(store_rows, Vec::new());
}

// ------------------------------------------------------------------- rows

#[test]
fn test_audit_rows_given_store_rows_returns_them_newest_first() -> Result<(), String> {
    // Arrange
    let roster = fixture_roster()?;
    let store_rows = vec![
        audit_row(9, "start", "", "0:00:00")?,
        audit_row(14, "dnf", "45", "mechanical failure")?,
        audit_row(11, "record_crossing", "45", "J. Okafor · solo")?,
    ];

    // Act
    let models = rows(&store_rows, &roster);

    // Assert
    assert_eq!(
        models
            .iter()
            .map(|row| row.action.as_str())
            .collect::<Vec<_>>(),
        vec!["dnf", "record_crossing", "start"]
    );
    Ok(())
}

#[test]
fn test_audit_rows_given_a_solo_plate_resolves_the_rider_name() -> Result<(), String> {
    // Arrange
    let roster = fixture_roster()?;
    let store_rows = vec![audit_row(11, "record_crossing", "45", "")?];

    // Act
    let models = rows(&store_rows, &roster);

    // Assert
    assert_eq!(
        models,
        vec![AuditRowModel {
            at: "11:00:00".to_owned(),
            action: "record_crossing".to_owned(),
            entry: "J. Okafor".to_owned(),
            entry_plate: "45".to_owned(),
            reason: String::new(),
        }]
    );
    Ok(())
}

#[test]
fn test_audit_rows_given_a_pooled_rider_plate_resolves_the_team_name() -> Result<(), String> {
    // Arrange
    let roster = fixture_roster()?;
    let store_rows = vec![audit_row(10, "edit_crossing", "77", "mis-keyed time")?];

    // Act
    let models = rows(&store_rows, &roster);

    // Assert
    assert_eq!(
        models
            .first()
            .map(|row| (row.entry.as_str(), row.entry_plate.as_str())),
        Some(("Trail Blazers", "77"))
    );
    Ok(())
}

#[test]
fn test_audit_rows_given_an_unresolved_entry_keeps_the_raw_cell() -> Result<(), String> {
    // Arrange
    let roster = fixture_roster()?;
    let store_rows = vec![audit_row(13, "shoe_reshuffle", "", "0 jokers added")?];

    // Act
    let models = rows(&store_rows, &roster);

    // Assert
    assert_eq!(
        models
            .first()
            .map(|row| (row.entry.as_str(), row.entry_plate.as_str())),
        Some(("", ""))
    );
    Ok(())
}

#[test]
fn test_audit_rows_given_no_rows_returns_an_empty_list() -> Result<(), String> {
    // Arrange
    let roster = fixture_roster()?;
    let store_rows: Vec<AuditRow> = Vec::new();

    // Act
    let models = rows(&store_rows, &roster);

    // Assert
    assert_eq!(models, Vec::new());
    Ok(())
}

// ----------------------------------------------------------------- filter

/// Return the mixed rows as view models over the fixture roster.
fn mixed_models() -> Result<Vec<AuditRowModel>, String> {
    Ok(rows(&mixed_rows()?, &fixture_roster()?))
}

#[test]
fn test_audit_filter_given_a_blank_search_and_all_actions_keeps_every_row() -> Result<(), String> {
    // Arrange
    let models = mixed_models()?;

    // Act
    let filtered = filter(&models, "  ", ALL_ACTIONS);

    // Assert
    assert_eq!(filtered, models);
    Ok(())
}

#[test]
fn test_audit_filter_given_a_plate_matches_that_entrys_rows() -> Result<(), String> {
    // Arrange
    let models = mixed_models()?;

    // Act
    let filtered = filter(&models, "45", ALL_ACTIONS);

    // Assert
    assert_eq!(
        filtered
            .iter()
            .map(|row| row.entry_plate.as_str())
            .collect::<Vec<_>>(),
        vec!["45", "45", "45"]
    );
    Ok(())
}

#[test]
fn test_audit_filter_given_a_lowercase_name_matches_case_insensitively() -> Result<(), String> {
    // Arrange
    let models = mixed_models()?;

    // Act
    let filtered = filter(&models, "okafor", ALL_ACTIONS);

    // Assert
    assert_eq!(
        filtered
            .iter()
            .map(|row| row.entry_plate.as_str())
            .collect::<Vec<_>>(),
        vec!["45", "45", "45"]
    );
    Ok(())
}

#[test]
fn test_audit_filter_given_a_team_name_matches_a_member_plate() -> Result<(), String> {
    // Arrange
    let models = mixed_models()?;

    // Act
    let filtered = filter(&models, "trail", ALL_ACTIONS);

    // Assert
    assert_eq!(
        filtered
            .iter()
            .map(|row| row.entry_plate.as_str())
            .collect::<Vec<_>>(),
        vec!["77"]
    );
    Ok(())
}

#[test]
fn test_audit_filter_never_matches_the_reason_or_the_action_text() -> Result<(), String> {
    // Arrange
    let models = mixed_models()?;

    // Act
    let by_reason = filter(&models, "mis-keyed", ALL_ACTIONS);
    let by_action = filter(&models, "edit_crossing", ALL_ACTIONS);

    // Assert
    assert_eq!(by_reason, Vec::new());
    assert_eq!(by_action, Vec::new());
    Ok(())
}

#[test]
fn test_audit_filter_given_an_action_keeps_only_that_actions_rows() -> Result<(), String> {
    // Arrange
    let models = mixed_models()?;

    // Act
    let filtered = filter(&models, "", "dnf");

    // Assert
    assert_eq!(
        filtered
            .iter()
            .map(|row| row.action.as_str())
            .collect::<Vec<_>>(),
        vec!["dnf"]
    );
    Ok(())
}

#[test]
fn test_audit_filter_given_an_unknown_action_keeps_nothing() -> Result<(), String> {
    // Arrange
    let models = mixed_models()?;

    // Act
    let filtered = filter(&models, "", "not_an_action");

    // Assert
    assert_eq!(filtered, Vec::new());
    Ok(())
}

#[test]
fn test_audit_filter_combines_the_search_and_the_action() -> Result<(), String> {
    // Arrange
    let models = mixed_models()?;

    // Act
    let filtered = filter(&models, "45", "record_crossing");

    // Assert
    assert_eq!(
        filtered
            .iter()
            .map(|row| row.action.as_str())
            .collect::<Vec<_>>(),
        vec!["record_crossing"]
    );
    Ok(())
}

// ---------------------------------------------------------- action choices

#[test]
fn test_audit_action_choices_given_the_canvas_order_has_29_unique_pairs() {
    // Arrange
    let labels: BTreeSet<&str> = ACTION_CHOICES.iter().map(|(label, _)| *label).collect();
    let actions: BTreeSet<&str> = ACTION_CHOICES.iter().map(|(_, action)| *action).collect();

    // Act
    let pairs = ACTION_CHOICES.len();

    // Assert
    assert_eq!(pairs, 29);
    assert_eq!(labels.len(), 29);
    assert_eq!(actions.len(), 29);
}

#[test]
fn test_audit_action_choices_given_the_canvas_order_starts_and_ends_as_declared() {
    // Arrange
    let expected_first = ("Record Crossing", "record_crossing");
    let expected_last = ("Reopen Ride", "reopen");

    // Act
    let first = ACTION_CHOICES.first().copied();
    let last = ACTION_CHOICES.last().copied();

    // Assert
    assert_eq!(first, Some(expected_first));
    assert_eq!(last, Some(expected_last));
}

#[test]
fn test_audit_action_choices_given_the_tiebreak_draw_offers_its_own_label() {
    // Arrange
    let expected = Some(("High-card Draw", "tiebreak_draw"));

    // Act
    let found = ACTION_CHOICES
        .iter()
        .find(|(_, action)| *action == "tiebreak_draw")
        .copied();

    // Assert
    assert_eq!(found, expected);
}

#[test]
fn test_audit_action_choices_never_carry_the_all_actions_label() {
    // Arrange
    let claimed = ACTION_CHOICES
        .iter()
        .any(|(label, action)| *label == ALL_ACTIONS || *action == ALL_ACTIONS);

    // Act
    let blank_label = ALL_ACTIONS.is_empty();

    // Assert
    assert!(!claimed);
    assert!(!blank_label);
}

#[test]
fn test_audit_action_choices_cover_every_audited_action_exactly_once() {
    // Arrange: the engine's 24 event actions plus the roster's 7 mutations,
    // with the two shared move spellings counted once.
    let expected: BTreeSet<&str> = [
        "record_crossing",
        "add_crossing_at",
        "edit_crossing",
        "void_crossing",
        "undo",
        "reassign",
        "record_miss",
        "assign_plate_to_miss",
        "deal_manual",
        "confirm_held",
        "void_held",
        "return_to_held",
        "void_card",
        "move_rider",
        "add_rider_to_team",
        "extract_rider_to_solo",
        "change_solo_plate",
        "change_pooled_rider_plate",
        "change_team_plate",
        "remove_rider",
        "dnf",
        "shoe_reshuffle",
        "tiebreak_draw",
        "start",
        "continue",
        "set_start_time",
        "stop",
        "finish",
        "reopen",
    ]
    .into_iter()
    .collect();
    let actual: BTreeSet<&str> = ACTION_CHOICES.iter().map(|(_, action)| *action).collect();

    // Act
    let missing: Vec<&str> = expected.difference(&actual).copied().collect();
    let extra: Vec<&str> = actual.difference(&expected).copied().collect();

    // Assert
    assert_eq!(missing, Vec::<&str>::new());
    assert_eq!(extra, Vec::<&str>::new());
}
