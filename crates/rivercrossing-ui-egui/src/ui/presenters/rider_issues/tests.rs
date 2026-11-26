// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the rider-issues dialog's rows, action gating and fixes.
//!
//! The rules and the copy follow the Python oracle's
//! `ui/presenters/rider_issues.py`; the families are ported by rule.

use pretty_assertions::assert_eq;
use rivercrossing_roster::enums::{EntryMode, EntryType, PlateModel, RideStatus};
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::{Entry, Rider};
use rstest::rstest;

use super::{
    IssueActions, IssueKind, IssueRow, actions, assign_plate, convert_to_solo, renumber, rows,
};

/// The seat seed every fixture ride stores.
const SEED: u64 = 7;

/// The stale-row refusal that every fix shares.
const STALE: &str = "the selected issue is no longer in the roster";

// ---------------------------------------------------------------- helpers

/// Return an empty mixed ride that pools rider plates.
fn pooled() -> Result<Roster, String> {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())
}

/// Return an empty mixed ride that plates whole teams.
fn relay() -> Result<Roster, String> {
    Roster::new(EntryMode::Mixed, 4, PlateModel::TeamRelay, Some(SEED))
        .map_err(|error| error.to_string())
}

/// Return one rider with the given name and own plate.
fn rider(first: &str, last: &str, plate: Option<&str>) -> Rider {
    Rider {
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        plate: plate.map(str::to_owned),
        ..Rider::default()
    }
}

/// Return a restored solo entry.
#[expect(
    clippy::too_many_arguments,
    reason = "one fixture call spells the entry, its plate and its rider"
)]
fn solo(key: &str, plate: &str, first: &str, last: &str, rider_plate: Option<&str>) -> Entry {
    let name = format!("{first} {last}").trim().to_owned();
    Entry::restored(
        key,
        plate,
        name,
        EntryType::Solo,
        vec![rider(first, last, rider_plate)],
    )
}

/// Return a restored team entry.
#[expect(
    clippy::too_many_arguments,
    reason = "one fixture call spells the team, its plate, its name and its members"
)]
fn team(key: &str, plate: &str, name: &str, members: Vec<Rider>) -> Entry {
    Entry::restored(key, plate, name, EntryType::Team, members)
}

/// Add one pooled team of a lone rider on *plate*.
fn add_team_of_one(roster: &mut Roster, name: &str, plate: &str) -> Result<(), String> {
    roster
        .create_team_entry_of_one(TeamFields {
            display_name: name,
            riders: vec![rider("Cy", "Three", Some(plate))],
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Return a roster that carries one of each supported defect.
fn defects() -> Result<Roster, String> {
    let mut roster = pooled()?;
    add_team_of_one(&mut roster, "Lone Wolf", "101")?;
    roster.load_entries(vec![
        solo("k-missing-name", "7", "", "", Some("7")),
        solo("k-missing-number", "9", "Grace", "Hopper", None),
        solo("k-dup-name-1", "11", "Ada", "Lovelace", Some("11")),
        solo("k-dup-name-2", "12", "Ada", "Lovelace", Some("12")),
        solo("k-dup-number-1", "21", "Bo", "One", Some("21")),
        solo("k-dup-number-2", "21", "Bo", "Two", Some("21")),
    ]);
    Ok(roster)
}

/// Return the row of *kind* that the roster reports.
fn find_row(roster: &Roster, kind: IssueKind) -> Result<IssueRow, String> {
    rows(roster)
        .into_iter()
        .find(|row| row.kind == kind)
        .ok_or_else(|| format!("no {kind:?} row"))
}

/// Return a row that denotes no reported issue.
fn stale_row(kind: IssueKind) -> IssueRow {
    IssueRow {
        kind,
        plate: "999".to_owned(),
        name: "Ghost".to_owned(),
        detail: "ghost".to_owned(),
    }
}

// ------------------------------------------------------------------ rows

#[test]
fn test_rows_lists_each_supported_defect_with_its_entry_plate_and_text() -> Result<(), String> {
    // Arrange
    let roster = defects()?;

    // Act
    let reported = rows(&roster);

    // Assert
    assert_eq!(
        reported,
        vec![
            IssueRow {
                kind: IssueKind::TeamOfOne,
                plate: "101".to_owned(),
                name: "Lone Wolf".to_owned(),
                detail: "team size must be at least 2, got 1".to_owned(),
            },
            IssueRow {
                kind: IssueKind::MissingName,
                plate: "7".to_owned(),
                name: String::new(),
                detail: "missing name".to_owned(),
            },
            IssueRow {
                kind: IssueKind::MissingNumber,
                plate: "9".to_owned(),
                name: "Grace Hopper".to_owned(),
                detail: "missing plate".to_owned(),
            },
            IssueRow {
                kind: IssueKind::DuplicateName,
                plate: "12".to_owned(),
                name: "Ada Lovelace".to_owned(),
                detail: "duplicate rider name Ada Lovelace".to_owned(),
            },
            IssueRow {
                kind: IssueKind::DuplicateNumber,
                plate: "21".to_owned(),
                name: "Bo Two".to_owned(),
                detail: "duplicate plate 21".to_owned(),
            },
        ]
    );
    Ok(())
}

#[test]
fn test_rows_skips_the_team_name_duplicate_kinds() -> Result<(), String> {
    // Arrange
    let mut roster = pooled()?;
    roster.load_entries(vec![
        team(
            "team-1",
            "1",
            "Alpha",
            vec![
                rider("Ada", "Lovelace", Some("1")),
                rider("Alan", "Turing", Some("2")),
            ],
        ),
        team(
            "team-2",
            "3",
            "alpha",
            vec![
                rider("Grace", "Hopper", Some("3")),
                rider("Edsger", "Dijkstra", Some("4")),
            ],
        ),
    ]);

    // Act
    let reported = rows(&roster);

    // Assert
    assert_eq!(reported, Vec::new());
    Ok(())
}

#[test]
fn test_rows_skips_the_near_duplicate_team_kind() -> Result<(), String> {
    // Arrange
    let mut roster = pooled()?;
    roster.load_entries(vec![
        team(
            "team-1",
            "1",
            "BNBA 1",
            vec![
                rider("Ada", "Lovelace", Some("1")),
                rider("Alan", "Turing", Some("2")),
            ],
        ),
        team(
            "team-2",
            "3",
            "BNBA1",
            vec![
                rider("Grace", "Hopper", Some("3")),
                rider("Edsger", "Dijkstra", Some("4")),
            ],
        ),
    ]);

    // Act
    let reported = rows(&roster);

    // Assert
    assert_eq!(reported, Vec::new());
    Ok(())
}

#[rstest]
#[case(IssueKind::TeamOfOne, "team-of-one")]
#[case(IssueKind::MissingName, "missing-name")]
#[case(IssueKind::MissingNumber, "missing-number")]
#[case(IssueKind::DuplicateName, "duplicate-name")]
#[case(IssueKind::DuplicateNumber, "duplicate-number")]
fn test_issue_kind_as_str_returns_the_oracle_spelling(
    #[case] kind: IssueKind,
    #[case] expected: &str,
) {
    // Arrange
    // (the case supplies the kind and its spelling)

    // Act
    let spelling = kind.as_str();

    // Assert
    assert_eq!(spelling, expected);
}

// --------------------------------------------------------------- actions

#[test]
fn test_actions_open_the_team_editor_for_a_team_of_one() -> Result<(), String> {
    // Arrange
    let roster = defects()?;
    let row = find_row(&roster, IssueKind::TeamOfOne)?;

    // Act
    let gate = actions(&roster, &row);

    // Assert
    assert_eq!(
        gate,
        IssueActions {
            open_editor: Some("team"),
            convert_solo: true,
            assign_plate: false,
            renumber: false,
        }
    );
    Ok(())
}

#[rstest]
#[case(IssueKind::MissingName)]
#[case(IssueKind::MissingNumber)]
#[case(IssueKind::DuplicateName)]
#[case(IssueKind::DuplicateNumber)]
fn test_actions_open_the_rider_editor_for_every_other_kind(
    #[case] kind: IssueKind,
) -> Result<(), String> {
    // Arrange
    let roster = defects()?;
    let row = find_row(&roster, kind)?;

    // Act
    let gate = actions(&roster, &row);

    // Assert
    assert_eq!(gate.open_editor, Some("rider"));
    Ok(())
}

#[test]
fn test_actions_enable_the_assign_plate_fix_for_a_missing_number() -> Result<(), String> {
    // Arrange
    let roster = defects()?;
    let row = find_row(&roster, IssueKind::MissingNumber)?;

    // Act
    let gate = actions(&roster, &row);

    // Assert
    assert_eq!(
        gate,
        IssueActions {
            open_editor: Some("rider"),
            convert_solo: false,
            assign_plate: true,
            renumber: false,
        }
    );
    Ok(())
}

#[test]
fn test_actions_enable_the_renumber_fix_for_a_duplicate_number() -> Result<(), String> {
    // Arrange
    let roster = defects()?;
    let row = find_row(&roster, IssueKind::DuplicateNumber)?;

    // Act
    let gate = actions(&roster, &row);

    // Assert
    assert_eq!(
        gate,
        IssueActions {
            open_editor: Some("rider"),
            convert_solo: false,
            assign_plate: false,
            renumber: true,
        }
    );
    Ok(())
}

#[test]
fn test_actions_given_an_empty_team_do_not_offer_the_conversion() -> Result<(), String> {
    // Arrange
    let mut roster = pooled()?;
    roster
        .create_empty_team(TeamFields {
            display_name: "Lone Wolf",
            riders: Vec::new(),
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    let row = find_row(&roster, IssueKind::TeamOfOne)?;

    // Act
    let gate = actions(&roster, &row);

    // Assert
    assert!(!gate.convert_solo);
    Ok(())
}

#[test]
fn test_actions_given_a_relay_ride_never_offer_the_conversion() -> Result<(), String> {
    // Arrange
    let mut roster = relay()?;
    roster
        .create_team_entry_of_one(TeamFields {
            display_name: "Lone Wolf",
            riders: vec![rider("Cy", "Three", None)],
            plate: Some("1"),
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    let row = find_row(&roster, IssueKind::TeamOfOne)?;

    // Act
    let gate = actions(&roster, &row);

    // Assert
    assert!(!gate.convert_solo);
    Ok(())
}

#[test]
fn test_actions_given_a_started_ride_disable_every_fix() -> Result<(), String> {
    // Arrange
    let mut roster = defects()?;
    let before = rows(&roster);
    roster.set_status(RideStatus::Running);

    // Act
    let gates: Vec<IssueActions> = before
        .iter()
        .filter(|row| row.kind != IssueKind::MissingName && row.kind != IssueKind::DuplicateName)
        .map(|row| actions(&roster, row))
        .collect();

    // Assert
    assert_eq!(
        gates
            .iter()
            .map(|gate| (gate.convert_solo, gate.assign_plate, gate.renumber))
            .collect::<Vec<_>>(),
        vec![
            (false, false, false),
            (false, false, false),
            (false, false, false)
        ]
    );
    Ok(())
}

// ------------------------------------------------------- convert to solo

#[test]
fn test_convert_to_solo_extracts_the_team_of_ones_only_rider() -> Result<(), String> {
    // Arrange
    let mut roster = pooled()?;
    add_team_of_one(&mut roster, "Lone Wolf", "101")?;
    let row = find_row(&roster, IssueKind::TeamOfOne)?;

    // Act
    let id = convert_to_solo(&mut roster, &row)?;

    // Assert
    assert_eq!(roster.entries().len(), 1);
    let extracted = roster
        .entry_by_key(&id.0)
        .ok_or("the conversion created no entry")?;
    assert_eq!(extracted.kind, EntryType::Solo);
    assert_eq!(extracted.plate, "101");
    Ok(())
}

#[test]
fn test_convert_to_solo_given_a_non_team_row_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = defects()?;
    let row = find_row(&roster, IssueKind::MissingNumber)?;

    // Act
    let result = convert_to_solo(&mut roster, &row);

    // Assert
    assert_eq!(
        result,
        Err("only a team-of-one can be converted to solo".to_owned())
    );
    Ok(())
}

#[test]
fn test_convert_to_solo_given_an_empty_team_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = pooled()?;
    roster
        .create_empty_team(TeamFields {
            display_name: "Lone Wolf",
            riders: Vec::new(),
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    let row = find_row(&roster, IssueKind::TeamOfOne)?;

    // Act
    let result = convert_to_solo(&mut roster, &row);

    // Assert
    assert_eq!(
        result,
        Err("an empty team has no rider to convert to solo".to_owned())
    );
    Ok(())
}

#[test]
fn test_convert_to_solo_given_a_relay_ride_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = relay()?;
    roster
        .create_team_entry_of_one(TeamFields {
            display_name: "Lone Wolf",
            riders: vec![rider("Cy", "Three", None)],
            plate: Some("1"),
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    let row = find_row(&roster, IssueKind::TeamOfOne)?;

    // Act
    let result = convert_to_solo(&mut roster, &row);

    // Assert
    assert_eq!(
        result,
        Err("convert to solo requires a rider-pooled ride".to_owned())
    );
    Ok(())
}

#[test]
fn test_convert_to_solo_given_a_started_ride_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = defects()?;
    let row = find_row(&roster, IssueKind::TeamOfOne)?;
    roster.set_status(RideStatus::Running);

    // Act
    let result = convert_to_solo(&mut roster, &row);

    // Assert
    assert_eq!(
        result,
        Err("a team-of-one can only be converted while the ride is draft".to_owned())
    );
    Ok(())
}

#[test]
fn test_convert_to_solo_given_a_stale_row_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = defects()?;
    let row = stale_row(IssueKind::TeamOfOne);

    // Act
    let result = convert_to_solo(&mut roster, &row);

    // Assert
    assert_eq!(result, Err(STALE.to_owned()));
    Ok(())
}

#[test]
fn test_convert_to_solo_given_a_plateless_rider_surfaces_the_roster_refusal() -> Result<(), String>
{
    // Arrange
    let mut roster = pooled()?;
    roster.load_entries(vec![team(
        "team-1",
        "101",
        "Lone Wolf",
        vec![rider("Cy", "Three", None)],
    )]);
    let row = find_row(&roster, IssueKind::TeamOfOne)?;

    // Act
    let result = convert_to_solo(&mut roster, &row);

    // Assert
    assert_eq!(
        result,
        Err("rider is not on any entry in this roster".to_owned())
    );
    Ok(())
}

// ----------------------------------------------------------- assign plate

#[test]
fn test_assign_plate_gives_a_missing_number_rider_the_supplied_plate() -> Result<(), String> {
    // Arrange
    let mut roster = pooled()?;
    roster.load_entries(vec![solo("k-missing-number", "9", "Grace", "Hopper", None)]);
    let row = find_row(&roster, IssueKind::MissingNumber)?;

    // Act
    assign_plate(&mut roster, &row, "19")?;

    // Assert
    let fixed = roster
        .entry_by_key("k-missing-number")
        .ok_or("the entry vanished")?;
    assert_eq!(fixed.plate, "19");
    assert_eq!(
        fixed
            .riders
            .first()
            .and_then(|member| member.plate.as_deref()),
        Some("19")
    );
    Ok(())
}

#[test]
fn test_assign_plate_given_a_non_missing_number_row_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = defects()?;
    let row = find_row(&roster, IssueKind::DuplicateNumber)?;

    // Act
    let result = assign_plate(&mut roster, &row, "19");

    // Assert
    assert_eq!(
        result,
        Err("select a missing-number issue to assign a plate".to_owned())
    );
    Ok(())
}

#[test]
fn test_assign_plate_off_draft_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = defects()?;
    let row = find_row(&roster, IssueKind::MissingNumber)?;
    roster.set_status(RideStatus::Running);

    // Act
    let result = assign_plate(&mut roster, &row, "19");

    // Assert
    assert_eq!(
        result,
        Err("plates can only be assigned while the ride is draft".to_owned())
    );
    Ok(())
}

#[test]
fn test_assign_plate_given_a_taken_plate_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled()?;
    roster.load_entries(vec![
        solo("k-missing-number", "9", "Grace", "Hopper", None),
        solo("k-other", "7", "Ada", "Lovelace", Some("7")),
    ]);
    let row = find_row(&roster, IssueKind::MissingNumber)?;

    // Act
    let result = assign_plate(&mut roster, &row, "7");

    // Assert
    assert_eq!(result, Err("plate '7' is already in use".to_owned()));
    Ok(())
}

#[test]
fn test_assign_plate_given_a_blank_plate_surfaces_the_shape_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled()?;
    roster.load_entries(vec![solo("k-missing-number", "9", "Grace", "Hopper", None)]);
    let row = find_row(&roster, IssueKind::MissingNumber)?;

    // Act
    let result = assign_plate(&mut roster, &row, "");

    // Assert
    assert_eq!(result, Err("plate '' must not be empty".to_owned()));
    Ok(())
}

#[test]
fn test_assign_plate_given_a_stale_row_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = defects()?;
    let row = stale_row(IssueKind::MissingNumber);

    // Act
    let result = assign_plate(&mut roster, &row, "19");

    // Assert
    assert_eq!(result, Err(STALE.to_owned()));
    Ok(())
}

// -------------------------------------------------------------- renumber

#[test]
fn test_renumber_moves_the_later_duplicate_claim_to_the_supplied_plate() -> Result<(), String> {
    // Arrange
    let mut roster = pooled()?;
    roster.load_entries(vec![
        solo("k-dup-number-1", "5", "Bo", "One", Some("5")),
        solo("k-dup-number-2", "5", "Bo", "Two", Some("5")),
    ]);
    let row = find_row(&roster, IssueKind::DuplicateNumber)?;

    // Act
    renumber(&mut roster, &row, "6")?;

    // Assert
    let later = roster
        .entry_by_key("k-dup-number-2")
        .ok_or("the later claim vanished")?;
    assert_eq!(later.plate, "6");
    assert_eq!(
        roster
            .entry_by_key("k-dup-number-1")
            .map(|earlier| earlier.plate.as_str()),
        Some("5")
    );
    Ok(())
}

#[test]
fn test_renumber_given_a_non_duplicate_number_row_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = defects()?;
    let row = find_row(&roster, IssueKind::MissingNumber)?;

    // Act
    let result = renumber(&mut roster, &row, "6");

    // Assert
    assert_eq!(
        result,
        Err("select a duplicate-number issue to renumber".to_owned())
    );
    Ok(())
}

#[test]
fn test_renumber_off_draft_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = defects()?;
    let row = find_row(&roster, IssueKind::DuplicateNumber)?;
    roster.set_status(RideStatus::Running);

    // Act
    let result = renumber(&mut roster, &row, "6");

    // Assert
    assert_eq!(
        result,
        Err("plates can only be changed while the ride is draft".to_owned())
    );
    Ok(())
}

#[test]
fn test_renumber_given_a_taken_plate_surfaces_the_refusal() -> Result<(), String> {
    // Arrange
    let mut roster = pooled()?;
    roster.load_entries(vec![
        solo("k-dup-number-1", "5", "Bo", "One", Some("5")),
        solo("k-dup-number-2", "5", "Bo", "Two", Some("5")),
        solo("k-other", "7", "Ada", "Lovelace", Some("7")),
    ]);
    let row = find_row(&roster, IssueKind::DuplicateNumber)?;

    // Act
    let result = renumber(&mut roster, &row, "7");

    // Assert
    assert_eq!(result, Err("plate '7' is already in use".to_owned()));
    Ok(())
}

#[test]
fn test_renumber_given_a_stale_row_is_refused() -> Result<(), String> {
    // Arrange
    let mut roster = defects()?;
    let row = stale_row(IssueKind::DuplicateNumber);

    // Act
    let result = renumber(&mut roster, &row, "6");

    // Assert
    assert_eq!(result, Err(STALE.to_owned()));
    Ok(())
}
