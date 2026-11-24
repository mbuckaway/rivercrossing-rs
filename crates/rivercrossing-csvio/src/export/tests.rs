// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the roster and standings CSV exporters.

use std::path::Path;

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;
use rivercrossing_hands::best_hand;
use rivercrossing_ride::EntryId;
use rivercrossing_roster::enums::{EntryMode, PlateModel, RideStatus};
use rivercrossing_roster::roster::{EntryEdit, Roster, TeamFields};
use rivercrossing_roster::types::Rider;
use rivercrossing_standings::{EntryResult, Placed, SnapshotKind};

use super::{export, export_standings};
use crate::error::CsvIoError;
use crate::model::ImportOptions;
use crate::preview::preview;

// ---------------------------------------------------------------- helpers

/// Build an empty mixed roster under *model*, accepting four-rider teams.
fn mixed(model: PlateModel) -> Roster {
    Roster::new(EntryMode::Mixed, 4, model, None).expect("the ride settings are valid")
}

/// Build a rider with the given name and sex, and no plate of their own.
fn rider(first: &str, last: &str, sex: Option<&str>) -> Rider {
    Rider {
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        sex: sex.map(str::to_owned),
        ..Rider::default()
    }
}

/// Return *rider* carrying *plate* as their own plate.
fn plated(mut rider: Rider, plate: &str) -> Rider {
    rider.plate = Some(plate.to_owned());
    rider
}

/// Add a solo entry for *rider* on *plate*.
fn add_solo(roster: &mut Roster, rider: Rider, plate: &str) {
    roster
        .create_solo_entry(rider, plate)
        .expect("the solo entry is valid");
}

/// Add the team that *fields* describes and return its id.
fn add_team(roster: &mut Roster, fields: TeamFields<'_>) -> EntryId {
    roster
        .create_team_entry(fields)
        .expect("the team entry is valid")
}

/// Build a team's fields with no logo card.
fn team_fields<'a>(name: &'a str, riders: Vec<Rider>, plate: Option<&'a str>) -> TeamFields<'a> {
    TeamFields {
        display_name: name,
        riders,
        plate,
        logo_card: None,
    }
}

/// Set the entry's operator notes.
fn note(roster: &mut Roster, id: &EntryId, notes: &str) {
    roster
        .update_entry(
            id,
            EntryEdit {
                notes: Some(notes),
                ..EntryEdit::default()
            },
        )
        .expect("the note is accepted");
}

/// Build a finished pooled roster holding one solo and one two-rider team.
fn finished_pooled() -> Roster {
    let mut roster = mixed(PlateModel::RiderPooled);
    add_solo(&mut roster, rider("Ada", "Lovelace", Some("F")), "1");
    let team = add_team(
        &mut roster,
        team_fields(
            "Byte Club",
            vec![
                plated(rider("Grace", "Hopper", Some("F")), "4"),
                plated(rider("Edsger", "Dijkstra", Some("M")), "3"),
            ],
            None,
        ),
    );
    note(&mut roster, &team, "Bench mob");
    roster.set_status(RideStatus::Finished);
    roster
}

/// Build a card from its code.
fn card(code: &str) -> Card {
    Card::parse(code).expect("the card code is valid")
}

/// Build the cards of *codes*.
fn cards(codes: &[&str]) -> Vec<Card> {
    codes.iter().copied().map(card).collect()
}

/// Build a zeroed solo reading on *plate*.
fn result(plate: &str, name: &str) -> EntryResult {
    EntryResult {
        entry_id: EntryId(plate.to_owned()),
        plate: plate.to_owned(),
        name: name.to_owned(),
        kind: SnapshotKind::Solo,
        laps: 0,
        total_time: 0.0,
        best_lap: 0.0,
        cards: Vec::new(),
        hand: best_hand(&[]),
        dnf: false,
        sex: None,
        tiebreak_card: None,
    }
}

/// Return *result* scored with the cards of *codes*.
fn with_cards(mut result: EntryResult, codes: &[&str]) -> EntryResult {
    let cards = cards(codes);
    result.hand = best_hand(&cards);
    result.cards = cards;
    result
}

/// Build one ranked row around *result*.
fn placed(result: EntryResult, place: Option<u32>) -> Placed {
    Placed {
        place,
        result,
        tie_note: None,
        draw_required: false,
    }
}

/// Return the export's bytes at *path*.
fn written(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    std::fs::read(path)
}

// ------------------------------------------------------- roster export

#[test]
fn test_export_pooled_draft_writes_one_row_per_rider() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("roster.csv");
    let mut roster = mixed(PlateModel::RiderPooled);
    add_solo(&mut roster, rider("Ada", "Lovelace", Some("F")), "1");
    add_solo(&mut roster, rider("Alan", "Turing", Some("M")), "2");
    let team = add_team(
        &mut roster,
        team_fields(
            "Byte Club",
            vec![
                plated(rider("Grace", "Hopper", Some("F")), "4"),
                plated(rider("Edsger", "Dijkstra", Some("M")), "3"),
            ],
            None,
        ),
    );
    note(&mut roster, &team, "Bench mob");

    export(&roster, &path, None)?;

    let expected: &[u8] = b"FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\r\n\
Ada,Lovelace,solo,,1,,F\r\n\
Alan,Turing,solo,,2,,M\r\n\
Grace,Hopper,team,Byte Club,4,Bench mob,F\r\n\
Edsger,Dijkstra,team,Byte Club,3,,M\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_relay_draft_repeats_the_entry_plate_on_team_rows()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("roster.csv");
    let mut roster = mixed(PlateModel::TeamRelay);
    add_solo(&mut roster, rider("Ada", "Lovelace", Some("F")), "1");
    add_solo(&mut roster, rider("Alan", "Turing", Some("M")), "2");
    let team = add_team(
        &mut roster,
        team_fields(
            "Byte Club",
            vec![
                rider("Grace", "Hopper", Some("F")),
                rider("Edsger", "Dijkstra", Some("M")),
            ],
            Some("3"),
        ),
    );
    note(&mut roster, &team, "Bench mob");

    export(&roster, &path, None)?;

    let expected: &[u8] = b"FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\r\n\
Ada,Lovelace,solo,,1,,F\r\n\
Alan,Turing,solo,,2,,M\r\n\
Grace,Hopper,team,Byte Club,3,Bench mob,F\r\n\
Edsger,Dijkstra,team,Byte Club,3,,M\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_finished_pooled_appends_stats_to_every_member_row()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("roster.csv");
    let roster = finished_pooled();
    let solo = with_cards(
        EntryResult {
            laps: 12,
            total_time: 3600.0,
            sex: Some("F".to_owned()),
            ..result("1", "Ada Lovelace")
        },
        &["9C", "9D", "2H", "4S", "7C"],
    );
    let team = with_cards(
        EntryResult {
            kind: SnapshotKind::Team,
            laps: 7,
            total_time: 1234.5,
            ..result("3", "Byte Club")
        },
        &["KC", "KD", "2H", "4S", "7C"],
    );
    let rows = vec![placed(solo, Some(1)), placed(team, Some(2))];

    export(&roster, &path, Some(&rows))?;

    // U+2014 separates each hand's class from its rank.
    let expected: &[u8] =
        b"FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX,laps,cards,best_hand,total_time\r\n\
Ada,Lovelace,solo,,1,,F,12,5,Pair \xE2\x80\x94 Nines,3600.0\r\n\
Grace,Hopper,team,Byte Club,4,Bench mob,F,7,5,Pair \xE2\x80\x94 Kings,1234.5\r\n\
Edsger,Dijkstra,team,Byte Club,3,,M,7,5,Pair \xE2\x80\x94 Kings,1234.5\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_finished_entry_without_cards_writes_a_blank_best_hand()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("roster.csv");
    let mut roster = mixed(PlateModel::RiderPooled);
    add_solo(&mut roster, rider("Ada", "Lovelace", Some("F")), "1");
    roster.set_status(RideStatus::Finished);
    let solo = EntryResult {
        laps: 3,
        total_time: 4210.5,
        sex: Some("F".to_owned()),
        ..result("1", "Ada Lovelace")
    };

    export(&roster, &path, Some(&[placed(solo, Some(1))]))?;

    let expected: &[u8] =
        b"FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX,laps,cards,best_hand,total_time\r\n\
Ada,Lovelace,solo,,1,,F,3,0,,4210.5\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_formula_leader_name_and_team_name_are_prefixed()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("roster.csv");
    let mut roster = mixed(PlateModel::RiderPooled);
    add_solo(&mut roster, rider("=SUM", "A1", None), "1");
    add_team(
        &mut roster,
        team_fields(
            "=2+2",
            vec![
                plated(rider("Bo", "Diddley", None), "2"),
                plated(rider("Sunny", "Day", None), "3"),
            ],
            None,
        ),
    );

    export(&roster, &path, None)?;

    let expected: &[u8] = b"FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\r\n\
'=SUM,A1,solo,,1,,\r\n\
Bo,Diddley,team,'=2+2,2,,\r\n\
Sunny,Day,team,'=2+2,3,,\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_pooled_leading_space_name_is_untouched() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("roster.csv");
    let mut roster = mixed(PlateModel::RiderPooled);
    add_solo(&mut roster, rider(" Ada", "Lovelace", None), "1");

    export(&roster, &path, None)?;

    let expected: &[u8] =
        b"FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\r\n Ada,Lovelace,solo,,1,,\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_unfinished_roster_returns_not_finished() {
    let dir = tempfile::TempDir::new().expect("the temporary directory is created");
    let path = dir.path().join("roster.csv");
    let roster = mixed(PlateModel::RiderPooled);

    let error = export(&roster, &path, Some(&[])).expect_err("a draft ride has no final standings");

    let CsvIoError::NotFinished { status } = error else {
        panic!("expected a not-finished error");
    };
    assert_eq!(status, "draft");
}

#[test]
fn test_export_finished_roster_missing_entry_standings_returns_missing_standings() {
    let dir = tempfile::TempDir::new().expect("the temporary directory is created");
    let path = dir.path().join("roster.csv");
    let roster = finished_pooled();
    let solo = placed(result("1", "Ada Lovelace"), Some(1));

    let error =
        export(&roster, &path, Some(&[solo])).expect_err("the team has no placed row in the slice");

    let CsvIoError::MissingStandings { plate } = error else {
        panic!("expected a missing-standings error");
    };
    assert_eq!(plate, "3");
}

#[test]
fn test_export_finished_roster_with_empty_placed_slice_returns_missing_standings() {
    let dir = tempfile::TempDir::new().expect("the temporary directory is created");
    let path = dir.path().join("roster.csv");
    let roster = finished_pooled();

    let error =
        export(&roster, &path, Some(&[])).expect_err("every entry needs a placed row in the slice");

    let CsvIoError::MissingStandings { plate } = error else {
        panic!("expected a missing-standings error");
    };
    assert_eq!(plate, "1");
}

#[test]
fn test_export_finished_empty_roster_with_empty_placed_slice_writes_the_header()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("roster.csv");
    let mut roster = mixed(PlateModel::RiderPooled);
    roster.set_status(RideStatus::Finished);

    export(&roster, &path, Some(&[]))?;

    let expected: &[u8] =
        b"FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX,laps,cards,best_hand,total_time\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_finished_roster_ignores_extra_placed_rows() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("roster.csv");
    let mut roster = mixed(PlateModel::RiderPooled);
    add_solo(&mut roster, rider("Ada", "Lovelace", Some("F")), "1");
    roster.set_status(RideStatus::Finished);
    let solo = EntryResult {
        laps: 2,
        total_time: 60.0,
        sex: Some("F".to_owned()),
        ..result("1", "Ada Lovelace")
    };
    let extra = result("99", "Ghost Rider");
    let rows = vec![placed(solo, Some(1)), placed(extra, Some(2))];

    export(&roster, &path, Some(&rows))?;

    let expected: &[u8] =
        b"FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX,laps,cards,best_hand,total_time\r\n\
Ada,Lovelace,solo,,1,,F,2,0,,60.0\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

// ----------------------------------------------------- standings export

/// Build the two ranked rows that the base standings tests pin.
fn standings_rows() -> Vec<Placed> {
    let ada = with_cards(
        EntryResult {
            laps: 12,
            total_time: 3600.0,
            sex: Some("F".to_owned()),
            ..result("7", "Ada Lovelace")
        },
        &["9C", "9D", "2H", "4S", "7C"],
    );
    let team = EntryResult {
        kind: SnapshotKind::Team,
        laps: 9,
        total_time: 5400.0,
        ..result("9", "Byte Club")
    };
    vec![placed(ada, Some(1)), placed(team, Some(2))]
}

#[test]
fn test_export_standings_writes_the_base_header_and_rows() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("standings.csv");

    export_standings(&standings_rows(), &path, false)?;

    let expected: &[u8] = b"place,plate,entry,type,sex,laps,hand,draw\r\n\
1,7,Ada Lovelace,solo,F,12,Pair \xE2\x80\x94 Nines,\r\n\
2,9,Byte Club,team,,9,,\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_standings_with_times_appends_the_time_column()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("standings.csv");

    export_standings(&standings_rows(), &path, true)?;

    let expected: &[u8] = b"place,plate,entry,type,sex,laps,hand,draw,total_time\r\n\
1,7,Ada Lovelace,solo,F,12,Pair \xE2\x80\x94 Nines,,3600.0\r\n\
2,9,Byte Club,team,,9,,,5400.0\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_standings_dns_row_writes_blank_place_and_dns_laps()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("standings.csv");
    let dns = EntryResult {
        sex: Some("F".to_owned()),
        ..result("12", "Zoe Quinn")
    };

    export_standings(&[placed(dns, None)], &path, false)?;

    let expected: &[u8] =
        b"place,plate,entry,type,sex,laps,hand,draw\r\n,12,Zoe Quinn,solo,F,DNS,,\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_standings_dns_row_with_times_leaves_the_time_blank()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("standings.csv");
    let dns = EntryResult {
        sex: Some("F".to_owned()),
        ..result("12", "Zoe Quinn")
    };

    export_standings(&[placed(dns, None)], &path, true)?;

    let expected: &[u8] =
        b"place,plate,entry,type,sex,laps,hand,draw,total_time\r\n,12,Zoe Quinn,solo,F,DNS,,,\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_standings_keeps_a_dnf_row() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("standings.csv");
    let dnf = EntryResult {
        kind: SnapshotKind::Team,
        dnf: true,
        laps: 4,
        total_time: 900.0,
        ..result("20", "Dana Scully")
    };

    export_standings(&[placed(dnf, Some(3))], &path, false)?;

    let expected: &[u8] = b"place,plate,entry,type,sex,laps,hand,draw\r\n\
3,20,Dana Scully,team,,4,,\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_standings_draw_card_writes_its_code() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("standings.csv");
    let ada = EntryResult {
        tiebreak_card: Some(card("KD")),
        laps: 12,
        ..result("7", "Ada Lovelace")
    };

    export_standings(&[placed(ada, Some(1))], &path, false)?;

    let expected: &[u8] = b"place,plate,entry,type,sex,laps,hand,draw\r\n\
1,7,Ada Lovelace,solo,,12,,KD\r\n";
    assert_eq!(written(&path)?, expected.to_vec());
    Ok(())
}

#[test]
fn test_export_pooled_round_trips_through_preview() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("pooled-draft.csv");
    let mut roster = mixed(PlateModel::RiderPooled);
    add_solo(&mut roster, rider("Ada", "Lovelace", Some("F")), "1");
    let team = add_team(
        &mut roster,
        team_fields(
            "Byte Club",
            vec![
                plated(rider("Grace", "Hopper", Some("F")), "4"),
                plated(rider("Edsger", "Dijkstra", Some("M")), "3"),
            ],
            None,
        ),
    );
    note(&mut roster, &team, "Bench mob");

    export(&roster, &path, None)?;
    let previewed = preview(&path, &roster, ImportOptions::default())?;

    assert_eq!(previewed.conflicts.len(), 0);
    assert_eq!(previewed.warnings.len(), 0);
    assert_eq!(previewed.riders, 3);
    assert_eq!(previewed.teams, 1);
    assert_eq!(previewed.entries.len(), 2);
    assert_eq!(previewed.entries[0].display_name, "Ada Lovelace");
    assert_eq!(previewed.entries[0].plate, "1");
    let reimported = &previewed.entries[1];
    assert_eq!(reimported.display_name, "Byte Club");
    assert_eq!(reimported.plate, "3");
    assert_eq!(reimported.notes, "Bench mob");
    assert_eq!(reimported.riders.len(), 2);
    Ok(())
}

#[test]
fn test_export_relay_round_trips_through_preview() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("relay-draft.csv");
    let mut roster = mixed(PlateModel::TeamRelay);
    add_solo(&mut roster, rider("Ada", "Lovelace", Some("F")), "1");
    let team = add_team(
        &mut roster,
        team_fields(
            "Byte Club",
            vec![
                rider("Grace", "Hopper", Some("F")),
                rider("Edsger", "Dijkstra", Some("M")),
            ],
            Some("5"),
        ),
    );
    note(&mut roster, &team, "Bench mob");

    export(&roster, &path, None)?;
    let previewed = preview(&path, &roster, ImportOptions::default())?;

    assert_eq!(previewed.conflicts.len(), 0);
    assert_eq!(previewed.warnings.len(), 0);
    assert_eq!(previewed.riders, 3);
    assert_eq!(previewed.teams, 1);
    assert_eq!(previewed.entries.len(), 2);
    assert_eq!(previewed.entries[1].plate, "5");
    assert_eq!(previewed.entries[1].display_name, "Byte Club");
    assert!(
        previewed.entries[1]
            .riders
            .iter()
            .all(|member| member.plate.is_none())
    );
    Ok(())
}
