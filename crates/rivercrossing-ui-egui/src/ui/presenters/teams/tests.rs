// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the teams editor's decisions.
//!
//! The rules and the copy follow the Python oracle's
//! `ui/presenters/teams.py`; the families are ported by rule.

use pretty_assertions::assert_eq;
use rivercrossing_roster::enums::{EntryMode, EntryType, PlateModel, RideStatus};
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::{Entry, EntryId, Rider};
use rstest::rstest;

use super::{
    TeamFormValues, TeamRow, add_team, edit_team, pick_card, remove_message, remove_team, rows,
};

/// The logo seed every fixture ride stores.
const SEED: u64 = 7;

/// The number of codes in one seeded card deck.
const DECK_CODES: usize = 52;

// ---------------------------------------------------------------- helpers

/// Return an empty mixed ride that pools rider plates.
fn pooled(max_team_size: u32) -> Result<Roster, String> {
    Roster::new(
        EntryMode::Mixed,
        max_team_size,
        PlateModel::RiderPooled,
        Some(SEED),
    )
    .map_err(|error| error.to_string())
}

/// Return an empty mixed ride with no logo deck.
fn without_deck() -> Result<Roster, String> {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, None)
        .map_err(|error| error.to_string())
}

/// Return an empty mixed ride that plates whole teams.
fn relay() -> Result<Roster, String> {
    Roster::new(EntryMode::Mixed, 4, PlateModel::TeamRelay, Some(SEED))
        .map_err(|error| error.to_string())
}

/// Return an empty ride whose entries are solo only.
fn solo_only() -> Result<Roster, String> {
    Roster::new(EntryMode::Solo, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())
}

/// Add one zero-rider team and return its id.
fn add_empty_team(roster: &mut Roster, name: &str) -> Result<EntryId, String> {
    roster
        .create_empty_team(TeamFields {
            display_name: name,
            riders: Vec::new(),
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())
}

/// Add one pooled team from its `(first, last, plate)` members.
fn add_pooled_team(
    roster: &mut Roster,
    name: &str,
    members: &[(&str, &str, &str)],
) -> Result<EntryId, String> {
    let riders = members
        .iter()
        .map(|(first, last, plate)| Rider {
            first_name: (*first).to_owned(),
            last_name: (*last).to_owned(),
            plate: Some((*plate).to_owned()),
            ..Rider::default()
        })
        .collect();
    roster
        .create_team_entry(TeamFields {
            display_name: name,
            riders,
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())
}

/// Add one relay team on *plate*.
#[expect(
    clippy::too_many_arguments,
    reason = "one fixture call spells the team, its plate and its two members"
)]
fn add_relay_team(
    roster: &mut Roster,
    name: &str,
    plate: &str,
    members: &[(&str, &str)],
) -> Result<EntryId, String> {
    let riders = members
        .iter()
        .map(|(first, last)| Rider {
            first_name: (*first).to_owned(),
            last_name: (*last).to_owned(),
            ..Rider::default()
        })
        .collect();
    roster
        .create_team_entry(TeamFields {
            display_name: name,
            riders,
            plate: Some(plate),
            logo_card: None,
        })
        .map_err(|error| error.to_string())
}

/// Add one solo entry.
#[expect(
    clippy::too_many_arguments,
    reason = "one fixture call spells the rider's name and plate"
)]
fn add_solo(roster: &mut Roster, first: &str, last: &str, plate: &str) -> Result<EntryId, String> {
    roster
        .create_solo_entry(
            Rider {
                first_name: first.to_owned(),
                last_name: last.to_owned(),
                ..Rider::default()
            },
            plate,
        )
        .map_err(|error| error.to_string())
}

/// Return the live entry that *id* names.
fn entry<'a>(roster: &'a Roster, id: &EntryId) -> Result<&'a Entry, String> {
    roster
        .entry_by_key(&id.0)
        .ok_or_else(|| format!("no entry {}", id.0))
}

/// Build a team form.
fn form(name: &str, relay_plate: &str, notes: &str) -> TeamFormValues {
    TeamFormValues {
        name: name.to_owned(),
        relay_plate: relay_plate.to_owned(),
        notes: notes.to_owned(),
    }
}

// ------------------------------------------------------------- add team

#[test]
fn test_add_team_creates_a_zero_rider_team_with_its_notes() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let expected_card = roster.next_team_logo_card(None);
    let values = form("Alpha", "", "Bench mob");

    // Act
    let id = add_team(&mut roster, &values)?;

    // Assert
    let created = entry(&roster, &id)?;
    assert_eq!(created.kind, EntryType::Team);
    assert_eq!(created.display_name, "Alpha");
    assert_eq!(created.plate, "1");
    assert_eq!(created.team_size(), 0);
    assert_eq!(created.notes, "Bench mob");
    assert_eq!(created.logo_card, expected_card);
    Ok(())
}

#[test]
fn test_add_team_given_no_notes_stores_none() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let values = form("Alpha", "", "");

    // Act
    let id = add_team(&mut roster, &values)?;

    // Assert
    assert_eq!(entry(&roster, &id)?.notes, "");
    Ok(())
}

#[rstest]
#[case("")]
#[case("   ")]
fn test_add_team_given_a_blank_name_is_refused(#[case] name: &str) -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let values = form(name, "", "");

    // Act
    let result = add_team(&mut roster, &values);

    // Assert
    assert_eq!(result, Err("enter a team name".to_owned()));
    assert_eq!(roster.entries().len(), 0);
    Ok(())
}

#[test]
fn test_add_team_given_a_duplicate_name_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Ada", "Lovelace", "101"), ("Alan", "Turing", "102")],
    )?;
    let values = form("  alpha  ", "", "");

    // Act
    let result = add_team(&mut roster, &values);

    // Assert
    assert_eq!(
        result,
        Err("a team named \"alpha\" already exists".to_owned())
    );
    assert_eq!(roster.entries().len(), 1);
    Ok(())
}

#[test]
fn test_add_team_on_a_solo_only_ride_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = solo_only()?;
    let values = form("Alpha", "", "");

    // Act
    let result = add_team(&mut roster, &values);

    // Assert
    assert_eq!(
        result,
        Err("this ride is solo-only; team entries are not allowed".to_owned())
    );
    Ok(())
}

#[test]
fn test_add_team_on_a_locked_ride_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    roster.set_status(RideStatus::Finished);
    let values = form("Alpha", "", "");

    // Act
    let result = add_team(&mut roster, &values);

    // Assert
    assert_eq!(
        result,
        Err("a new team cannot be started once the ride is finished".to_owned())
    );
    Ok(())
}

#[test]
fn test_add_team_on_a_relay_ride_takes_the_relay_plate() -> Result<(), String> {
    // Arrange
    let mut roster = relay()?;
    let values = form("Alpha", "7", "");

    // Act
    let id = add_team(&mut roster, &values)?;

    // Assert
    assert_eq!(entry(&roster, &id)?.plate, "7");
    Ok(())
}

#[test]
fn test_add_team_on_a_relay_ride_given_a_blank_plate_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = relay()?;
    let values = form("Alpha", "  ", "");

    // Act
    let result = add_team(&mut roster, &values);

    // Assert
    assert_eq!(result, Err("plate '  ' must not be empty".to_owned()));
    Ok(())
}

// ------------------------------------------------------------ edit team

#[test]
fn test_edit_team_renames_the_team_and_stores_its_notes() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_empty_team(&mut roster, "Alpha")?;
    let values = form("Bravo", "", "Bench mob");

    // Act
    edit_team(&mut roster, &id, &values)?;

    // Assert
    let edited = entry(&roster, &id)?;
    assert_eq!(edited.display_name, "Bravo");
    assert_eq!(edited.notes, "Bench mob");
    Ok(())
}

#[test]
fn test_edit_team_given_an_unchanged_form_writes_nothing() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_empty_team(&mut roster, "Alpha")?;
    let audit_len = roster.audit_log().len();
    let values = form("Alpha", "", "");

    // Act
    edit_team(&mut roster, &id, &values)?;

    // Assert
    assert_eq!(roster.audit_log().len(), audit_len);
    Ok(())
}

#[test]
fn test_edit_team_allows_recasing_its_own_name() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_empty_team(&mut roster, "Alpha")?;
    let values = form("ALPHA", "", "");

    // Act
    edit_team(&mut roster, &id, &values)?;

    // Assert
    assert_eq!(entry(&roster, &id)?.display_name, "ALPHA");
    Ok(())
}

#[test]
fn test_edit_team_given_another_teams_name_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let alpha = add_empty_team(&mut roster, "Alpha")?;
    add_empty_team(&mut roster, "Bravo")?;
    let values = form("Bravo", "", "");

    // Act
    let result = edit_team(&mut roster, &alpha, &values);

    // Assert
    assert_eq!(
        result,
        Err("a team named \"Bravo\" already exists".to_owned())
    );
    assert_eq!(entry(&roster, &alpha)?.display_name, "Alpha");
    Ok(())
}

#[test]
fn test_edit_team_on_a_relay_ride_replates_the_team() -> Result<(), String> {
    // Arrange
    let mut roster = relay()?;
    let alpha = add_relay_team(
        &mut roster,
        "Alpha",
        "1",
        &[("Ada", "Lovelace"), ("Alan", "Turing")],
    )?;
    let values = form("Alpha", "9", "");

    // Act
    edit_team(&mut roster, &alpha, &values)?;

    // Assert
    assert_eq!(entry(&roster, &alpha)?.plate, "9");
    Ok(())
}

#[test]
fn test_edit_team_given_a_duplicate_relay_plate_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = relay()?;
    let alpha = add_relay_team(
        &mut roster,
        "Alpha",
        "1",
        &[("Ada", "Lovelace"), ("Alan", "Turing")],
    )?;
    add_relay_team(
        &mut roster,
        "Bravo",
        "2",
        &[("Grace", "Hopper"), ("Edsger", "Dijkstra")],
    )?;
    let values = form("Alpha", "2", "");

    // Act
    let result = edit_team(&mut roster, &alpha, &values);

    // Assert
    assert_eq!(result, Err("plate '2' is already in use".to_owned()));
    Ok(())
}

#[test]
fn test_edit_team_given_a_blank_name_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_empty_team(&mut roster, "Alpha")?;
    let values = form("  ", "", "");

    // Act
    let result = edit_team(&mut roster, &id, &values);

    // Assert
    assert_eq!(result, Err("enter a team name".to_owned()));
    Ok(())
}

#[test]
fn test_edit_team_given_an_unknown_entry_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let missing = EntryId("missing".to_owned());
    let values = form("Bravo", "", "");

    // Act
    let result = edit_team(&mut roster, &missing, &values);

    // Assert
    assert_eq!(
        result,
        Err("entry missing is not a member of this roster".to_owned())
    );
    Ok(())
}

// ------------------------------------------------------------- pick card

#[test]
fn test_pick_card_draws_a_seeded_card_and_stores_it() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let alpha = add_empty_team(&mut roster, "Alpha")?;

    // Act
    let code = pick_card(&mut roster, &alpha, SEED)?;

    // Assert
    assert_eq!(
        entry(&roster, &alpha)?.logo_card.as_deref(),
        Some(code.as_str())
    );
    Ok(())
}

#[test]
fn test_pick_card_given_a_claimed_card_draws_another() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let alpha = add_empty_team(&mut roster, "Alpha")?;
    let bravo = add_empty_team(&mut roster, "Bravo")?;

    // Act
    let first = pick_card(&mut roster, &alpha, SEED)?;
    let second = pick_card(&mut roster, &bravo, SEED)?;

    // Assert
    assert_ne!(first, second);
    Ok(())
}

#[test]
fn test_pick_card_given_no_deck_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = without_deck()?;
    let alpha = add_empty_team(&mut roster, "Alpha")?;

    // Act
    let result = pick_card(&mut roster, &alpha, SEED);

    // Assert
    assert_eq!(
        result,
        Err("no card deck is available for this ride".to_owned())
    );
    Ok(())
}

#[test]
fn test_pick_card_given_every_card_claimed_is_exhausted() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let alpha = add_empty_team(&mut roster, "Alpha")?;
    exhaust_logo_deck(&mut roster)?;

    // Act
    let result = pick_card(&mut roster, &alpha, SEED);

    // Assert
    assert_eq!(
        result,
        Err("every card logo is already in use by a team".to_owned())
    );
    Ok(())
}

/// Add one team per deck code, so every logo card is claimed.
fn exhaust_logo_deck(roster: &mut Roster) -> Result<(), String> {
    for index in 0..DECK_CODES {
        add_empty_team(roster, &format!("Fill{index}"))?;
    }
    Ok(())
}

#[test]
fn test_pick_card_given_an_unknown_entry_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let missing = EntryId("missing".to_owned());

    // Act
    let result = pick_card(&mut roster, &missing, SEED);

    // Assert
    assert_eq!(
        result,
        Err("entry missing is not a member of this roster".to_owned())
    );
    Ok(())
}

// ----------------------------------------------------------------- rows

#[test]
fn test_team_rows_list_every_team_with_its_plate_and_size() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Ada", "Lovelace", "101"), ("Alan", "Turing", "102")],
    )?;
    add_empty_team(&mut roster, "Bravo")?;

    // Act
    let rendered = rows(&roster, false);

    // Assert
    assert_eq!(
        rendered,
        vec![
            TeamRow {
                name: "Alpha".to_owned(),
                plate: "101".to_owned(),
                members: 2,
            },
            TeamRow {
                name: "Bravo".to_owned(),
                plate: "103".to_owned(),
                members: 0,
            },
        ]
    );
    Ok(())
}

#[test]
fn test_team_rows_given_single_member_only_hides_larger_teams() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    add_pooled_team(
        &mut roster,
        "Alpha",
        &[("Ada", "Lovelace", "101"), ("Alan", "Turing", "102")],
    )?;
    roster
        .create_team_entry_of_one(TeamFields {
            display_name: "Bravo",
            riders: vec![Rider {
                first_name: "Grace".to_owned(),
                last_name: "Hopper".to_owned(),
                plate: Some("201".to_owned()),
                ..Rider::default()
            }],
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;

    // Act
    let rendered = rows(&roster, true);

    // Assert
    assert_eq!(
        rendered
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Bravo"]
    );
    Ok(())
}

#[test]
fn test_team_rows_never_lists_a_solo_entry() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    add_solo(&mut roster, "Ada", "Lovelace", "101")?;
    add_empty_team(&mut roster, "Alpha")?;

    // Act
    let rendered = rows(&roster, false);

    // Assert
    assert_eq!(
        rendered
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Alpha"]
    );
    Ok(())
}

// --------------------------------------------------------------- remove

#[test]
fn test_remove_message_names_the_team() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_empty_team(&mut roster, "Alpha")?;
    let target = entry(&roster, &id)?;

    // Act
    let message = remove_message(target);

    // Assert
    assert_eq!(message, "Remove the team \"Alpha\"?");
    Ok(())
}

#[test]
fn test_remove_team_deletes_the_entry() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_empty_team(&mut roster, "Alpha")?;

    // Act
    remove_team(&mut roster, &id)?;

    // Assert
    assert_eq!(roster.entries().len(), 0);
    Ok(())
}

#[test]
fn test_remove_team_given_recorded_data_surfaces_the_lock() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let id = add_empty_team(&mut roster, "Alpha")?;
    roster
        .mark_has_data(&id)
        .map_err(|error| error.to_string())?;

    // Act
    let result = remove_team(&mut roster, &id);

    // Assert
    assert_eq!(
        result,
        Err("entry has recorded data; DNF or void it instead of deleting".to_owned())
    );
    Ok(())
}

#[test]
fn test_remove_team_given_an_unknown_entry_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled(4)?;
    let missing = EntryId("missing".to_owned());

    // Act
    let result = remove_team(&mut roster, &missing);

    // Assert
    assert_eq!(
        result,
        Err("entry missing is not a member of this roster".to_owned())
    );
    Ok(())
}
