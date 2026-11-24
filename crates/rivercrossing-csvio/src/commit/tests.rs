// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the atomic CSV import commit.

use std::path::Path;

use pretty_assertions::assert_eq;
use rivercrossing_roster::enums::{EntryMode, EntryType, PlateModel};
use rivercrossing_roster::error::RosterError;
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::{AuditEvent, Entry, Rider};
use rstest::rstest;

use super::commit;
use crate::error::CsvIoError;
use crate::model::{ImportOptions, ImportPreview, ImportReport, ProblemCode};
use crate::preview::preview_bytes;

/// The source path that every inline test previews under.
const SOURCE: &str = "entries.csv";

/// The canonical export header, in the column order the tests rely on.
const HEADER: &str = "FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\n";

/// One entry's fully compared state.
#[derive(Debug, PartialEq)]
struct EntryView {
    /// The stable identity.
    key: String,
    /// The entry's plate.
    plate: String,
    /// The name that the entry list shows.
    display_name: String,
    /// The entry's kind.
    kind: EntryType,
    /// The operator's notes.
    notes: String,
    /// Each rider's names, own plate and sex.
    riders: Vec<(String, String, Option<String>, Option<String>)>,
}

// ---------------------------------------------------------------- helpers

/// Return *rows* under the canonical header.
fn csv(rows: &str) -> String {
    format!("{HEADER}{rows}")
}

/// Return the default import options.
fn options() -> ImportOptions {
    ImportOptions::default()
}

/// Return the options that turn a one-rider team into a solo entry.
fn convert_only() -> ImportOptions {
    ImportOptions {
        map_unknown_sex_to_male: false,
        convert_teams_of_one_to_solo: true,
    }
}

/// Build a draft roster under the mixed entry mode, accepting four-rider teams.
fn roster_of(model: PlateModel, entries: Vec<Entry>) -> Roster {
    let mut roster = Roster::new(EntryMode::Mixed, 4, model, None).expect("the settings are valid");
    roster.load_entries(entries);
    roster
}

/// Build a rider with the given names and own plate.
fn rider(first: &str, last: &str, plate: Option<&str>) -> Rider {
    Rider {
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        plate: plate.map(str::to_owned),
        ..Rider::default()
    }
}

/// Build a solo entry with the given names and own plate.
#[expect(
    clippy::too_many_arguments,
    reason = "the fixture names the entry's key, plate, rider and own plate; a struct would obscure the case"
)]
fn entry_solo(key: &str, plate: &str, name: (&str, &str), own_plate: Option<&str>) -> Entry {
    let (first, last) = name;
    Entry::restored(
        key,
        plate,
        format!("{first} {last}"),
        EntryType::Solo,
        vec![rider(first, last, own_plate)],
    )
}

/// Build a team entry with the given members and their own plates.
#[expect(
    clippy::too_many_arguments,
    reason = "the fixture names the entry's key, plate, name and members; a struct would obscure the case"
)]
fn entry_team(
    key: &str,
    plate: &str,
    display_name: &str,
    members: Vec<(&str, &str, Option<&str>)>,
) -> Entry {
    let riders = members
        .into_iter()
        .map(|(first, last, own_plate)| rider(first, last, own_plate))
        .collect();
    Entry::restored(key, plate, display_name, EntryType::Team, riders)
}

/// Return the own plate that *model* gives a solo entry's rider.
fn own_plate(model: PlateModel, plate: &str) -> Option<&str> {
    if model == PlateModel::RiderPooled {
        return Some(plate);
    }
    None
}

/// Preview *text* against *roster* with the default options.
fn preview_on(roster: &Roster, text: &str) -> ImportPreview {
    preview_bytes(Path::new(SOURCE), text.as_bytes(), roster, options())
        .expect("the payload previews")
}

/// Preview *text* against *roster* with *options*.
fn preview_with(roster: &Roster, text: &str, options: ImportOptions) -> ImportPreview {
    preview_bytes(Path::new(SOURCE), text.as_bytes(), roster, options)
        .expect("the payload previews")
}

/// Return the report's five counts.
fn counts(report: &ImportReport) -> (usize, usize, usize, usize, usize) {
    (
        report.inserted,
        report.updated,
        report.moved,
        report.extracted,
        report.joined,
    )
}

/// Return the report's audit actions, in order.
fn actions(report: &ImportReport) -> Vec<&str> {
    report
        .audit_events
        .iter()
        .map(|event| event.action.as_str())
        .collect()
}

/// Return every entry's fully compared state.
fn views(roster: &Roster) -> Vec<EntryView> {
    roster
        .entries()
        .iter()
        .map(|entry| EntryView {
            key: entry.key.clone(),
            plate: entry.plate.clone(),
            display_name: entry.display_name.clone(),
            kind: entry.kind,
            notes: entry.notes.clone(),
            riders: entry
                .riders
                .iter()
                .map(|rider| {
                    (
                        rider.first_name.clone(),
                        rider.last_name.clone(),
                        rider.plate.clone(),
                        rider.sex.clone(),
                    )
                })
                .collect(),
        })
        .collect()
}

/// Return the roster's entries and its audit log.
fn snapshot(roster: &Roster) -> (Vec<EntryView>, Vec<AuditEvent>) {
    (views(roster), roster.audit_log().to_vec())
}

/// Return the roster's rider full names per entry.
fn rider_names(roster: &Roster) -> Vec<Vec<String>> {
    roster
        .entries()
        .iter()
        .map(|entry| entry.riders.iter().map(Rider::full_name).collect())
        .collect()
}

// ============================================================ relay insert

#[test]
fn test_commit_relay_given_a_new_solo_inserts_it_with_its_plate() {
    let mut roster = roster_of(PlateModel::TeamRelay, Vec::new());
    let preview = preview_on(&roster, &csv("Ada,Lovelace,solo,,7,,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (1, 0, 0, 0, 0));
    assert_eq!(actions(&report), vec!["create_solo_entry"]);
    assert_eq!(
        roster.resolve_plate("7").map(|entry| (
            entry.display_name.as_str(),
            entry.riders.len(),
            entry
                .riders
                .first()
                .and_then(|rider| rider.plate.as_deref()),
        )),
        Some(("Ada Lovelace", 1, None))
    );
}

#[test]
fn test_commit_relay_given_a_one_rider_team_inserts_a_team_of_one() {
    let mut roster = roster_of(PlateModel::TeamRelay, Vec::new());
    let preview = preview_on(&roster, &csv("Ada,One,team,Wolves,5,,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(preview.conflicts, Vec::new());
    assert_eq!(
        preview
            .warnings
            .iter()
            .map(|warning| (warning.row, warning.code.clone()))
            .collect::<Vec<_>>(),
        vec![(2, ProblemCode::TeamUnderMin)]
    );
    assert_eq!(counts(&report), (1, 0, 0, 0, 0));
    assert_eq!(actions(&report), vec!["create_team_entry_of_one"]);
    assert_eq!(
        views(&roster)
            .iter()
            .map(|entry| (
                entry.display_name.as_str(),
                entry.kind,
                entry.plate.as_str(),
                entry.riders.len()
            ))
            .collect::<Vec<_>>(),
        vec![("Wolves", EntryType::Team, "5", 1)]
    );
}

#[test]
fn test_commit_relay_given_a_changed_team_reduced_to_one_rider_recreates_it() {
    let mut roster = roster_of(
        PlateModel::TeamRelay,
        vec![entry_team(
            "t",
            "5",
            "Wolves",
            vec![("Ada", "One", None), ("Bo", "Two", None)],
        )],
    );
    let preview = preview_on(&roster, &csv("Ada,One,team,Wolves,5,,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(preview.conflicts, Vec::new());
    assert_eq!(counts(&report), (0, 1, 0, 0, 0));
    assert_eq!(
        actions(&report),
        vec!["delete_entry", "create_team_entry_of_one"]
    );
    assert_eq!(
        views(&roster)
            .iter()
            .map(|entry| (
                entry.display_name.as_str(),
                entry.kind,
                entry.plate.as_str(),
                entry.riders.len()
            ))
            .collect::<Vec<_>>(),
        vec![("Wolves", EntryType::Team, "5", 1)]
    );
}

#[test]
fn test_commit_relay_given_a_new_team_inserts_it_with_its_plate_and_notes() {
    let mut roster = roster_of(PlateModel::TeamRelay, Vec::new());
    let preview = preview_on(
        &roster,
        &csv("Ada,One,team,Wolves,5,hi,\nBo,Two,team,Wolves,5,,\n"),
    );

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (1, 0, 0, 0, 0));
    assert_eq!(actions(&report), vec!["create_team_entry", "update_entry"]);
    assert_eq!(
        roster.resolve_plate("5").map(|entry| (
            entry.display_name.as_str(),
            entry.kind,
            entry.notes.as_str(),
            entry
                .riders
                .iter()
                .map(|rider| rider.plate.clone())
                .collect::<Vec<_>>(),
        )),
        Some(("Wolves", EntryType::Team, "hi", vec![None, None]))
    );
}

// ============================================================= relay edits

#[test]
fn test_commit_relay_given_a_matching_solo_renames_it_and_updates_the_notes() {
    let mut roster = roster_of(
        PlateModel::TeamRelay,
        vec![entry_solo("s", "7", ("Ada", "Lovelace"), None)],
    );
    let preview = preview_on(&roster, &csv("Ada,Byron,solo,,7,note,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 1, 0, 0, 0));
    assert_eq!(actions(&report), vec!["update_rider", "update_entry"]);
    assert_eq!(
        roster
            .resolve_plate("7")
            .map(|entry| (entry.display_name.as_str(), entry.notes.as_str())),
        Some(("Ada Byron", "note"))
    );
}

#[test]
fn test_commit_relay_given_a_matching_solo_sex_change_updates_the_rider() {
    let mut roster = roster_of(
        PlateModel::TeamRelay,
        vec![entry_solo("s", "7", ("Ada", "Lovelace"), None)],
    );
    let preview = preview_on(&roster, &csv("Ada,Lovelace,solo,,7,,M\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 1, 0, 0, 0));
    assert_eq!(actions(&report), vec!["update_rider"]);
    assert_eq!(
        roster
            .resolve_plate("7")
            .and_then(|entry| entry.riders.first())
            .and_then(|rider| rider.sex.as_deref()),
        Some("M")
    );
}

#[test]
fn test_commit_relay_given_a_changed_team_recreates_it() {
    let mut roster = roster_of(
        PlateModel::TeamRelay,
        vec![entry_team(
            "t",
            "5",
            "Wolves",
            vec![("Ada", "Lovelace", None), ("Alan", "Turing", None)],
        )],
    );
    let preview = preview_on(
        &roster,
        &csv("Grace,Hopper,team,Wolves,5,,\nAlan,Turing,team,Wolves,5,,\n"),
    );

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 1, 0, 0, 0));
    assert_eq!(actions(&report), vec!["delete_entry", "create_team_entry"]);
    assert_eq!(
        roster
            .entries()
            .iter()
            .map(|entry| (entry.key.as_str() == "t", entry.plate.as_str()))
            .collect::<Vec<_>>(),
        vec![(false, "5")]
    );
    assert_eq!(
        rider_names(&roster),
        vec![vec!["Grace Hopper".to_owned(), "Alan Turing".to_owned()]]
    );
}

#[test]
fn test_commit_relay_given_a_team_turned_solo_recreates_it() {
    let mut roster = roster_of(
        PlateModel::TeamRelay,
        vec![entry_team(
            "t",
            "5",
            "Wolves",
            vec![("Ada", "One", None), ("Bo", "Two", None)],
        )],
    );
    let preview = preview_on(&roster, &csv("Ada,One,solo,,5,,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 1, 0, 0, 0));
    assert_eq!(actions(&report), vec!["delete_entry", "create_solo_entry"]);
    assert_eq!(
        roster.resolve_plate("5").map(|entry| (
            entry.kind,
            entry.display_name.as_str(),
            entry.riders.len()
        )),
        Some((EntryType::Solo, "Ada One", 1))
    );
}

#[test]
fn test_commit_relay_given_a_matching_team_updates_the_sexes_and_notes() {
    let mut roster = roster_of(
        PlateModel::TeamRelay,
        vec![entry_team(
            "t",
            "5",
            "Wolves",
            vec![("Ada", "Lovelace", None), ("Alan", "Turing", None)],
        )],
    );
    let preview = preview_on(
        &roster,
        &csv("Ada,Lovelace,team,Wolves,5,hi,M\nAlan,Turing,team,Wolves,5,,F\n"),
    );

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 1, 0, 0, 0));
    assert_eq!(
        actions(&report),
        vec!["update_rider", "update_rider", "update_entry"]
    );
    assert_eq!(
        roster.resolve_plate("5").map(|entry| (
            entry.notes.as_str(),
            entry
                .riders
                .iter()
                .map(|rider| rider.sex.clone())
                .collect::<Vec<_>>(),
        )),
        Some(("hi", vec![Some("M".to_owned()), Some("F".to_owned())]))
    );
}

#[test]
fn test_commit_relay_given_the_same_file_twice_changes_nothing_on_the_second_import() {
    let mut roster = roster_of(PlateModel::TeamRelay, Vec::new());
    let text = csv("Ada,Lovelace,solo,,7,hi,\nBo,Two,team,Wolves,5,,\nCy,Three,team,Wolves,5,,\n");
    let first = preview_on(&roster, &text);
    let first_report = commit(&mut roster, &first).expect("the first import commits");
    let audit_after_first = roster.audit_log().len();

    let second = preview_on(&roster, &text);
    let second_report = commit(&mut roster, &second).expect("the second import commits");

    assert_eq!(counts(&first_report), (2, 0, 0, 0, 0));
    assert_eq!(counts(&second_report), (0, 0, 0, 0, 0));
    assert_eq!(second_report.audit_events, Vec::new());
    assert_eq!(roster.audit_log().len(), audit_after_first);
}

#[rstest]
#[case(PlateModel::TeamRelay)]
#[case(PlateModel::RiderPooled)]
fn test_commit_given_a_one_rider_team_and_the_convert_opt_in_inserts_a_solo(
    #[case] model: PlateModel,
) {
    let mut roster = roster_of(model, Vec::new());
    let preview = preview_with(&roster, &csv("Ada,One,team,Wolves,5,,\n"), convert_only());

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (1, 0, 0, 0, 0));
    assert_eq!(actions(&report), vec!["create_solo_entry"]);
    assert_eq!(
        roster
            .resolve_plate("5")
            .map(|entry| (entry.kind, entry.riders.len())),
        Some((EntryType::Solo, 1))
    );
}

// ============================================================ pooled solo

#[test]
fn test_commit_pooled_given_a_new_solo_inserts_it_and_sets_the_notes() {
    let mut roster = roster_of(PlateModel::RiderPooled, Vec::new());
    let preview = preview_on(&roster, &csv("Ada,One,solo,,7,hi,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (1, 0, 0, 0, 0));
    assert_eq!(actions(&report), vec!["create_solo_entry", "update_entry"]);
    assert_eq!(
        roster.resolve_plate("7").map(|entry| (
            entry.display_name.as_str(),
            entry.notes.as_str(),
            entry
                .riders
                .first()
                .and_then(|rider| rider.plate.as_deref()),
        )),
        Some(("Ada One", "hi", Some("7")))
    );
}

#[test]
fn test_commit_pooled_given_a_matching_solo_renames_it_and_updates_the_notes() {
    let mut roster = roster_of(
        PlateModel::RiderPooled,
        vec![entry_solo("s", "1", ("Ada", "One"), Some("1"))],
    );
    let preview = preview_on(&roster, &csv("Ada,Byron,solo,,1,hi,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 1, 0, 0, 0));
    assert_eq!(actions(&report), vec!["update_rider", "update_entry"]);
    assert_eq!(
        roster
            .resolve_plate("1")
            .map(|entry| (entry.display_name.as_str(), entry.notes.as_str())),
        Some(("Ada Byron", "hi"))
    );
}

#[test]
fn test_commit_pooled_given_a_team_member_turned_solo_extracts_it() {
    let mut roster = roster_of(
        PlateModel::RiderPooled,
        vec![entry_team(
            "t",
            "1",
            "Wolves",
            vec![("Ada", "One", Some("1")), ("Bo", "Two", Some("2"))],
        )],
    );
    let preview = preview_on(&roster, &csv("Ada,One,solo,,1,,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 0, 0, 1, 0));
    assert_eq!(actions(&report), vec!["extract_rider_to_solo"]);
    assert_eq!(
        roster
            .resolve_plate("1")
            .map(|entry| (entry.kind, entry.display_name.as_str())),
        Some((EntryType::Solo, "Ada One"))
    );
}

#[test]
fn test_commit_pooled_given_an_extracted_member_renamed_updates_it() {
    let mut roster = roster_of(
        PlateModel::RiderPooled,
        vec![entry_team(
            "t",
            "1",
            "Wolves",
            vec![("Ada", "One", Some("1")), ("Bo", "Two", Some("2"))],
        )],
    );
    let preview = preview_on(&roster, &csv("Ada,Byron,solo,,1,hi,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 1, 0, 1, 0));
    assert_eq!(
        actions(&report),
        vec!["extract_rider_to_solo", "update_rider", "update_entry"]
    );
    assert_eq!(
        roster
            .resolve_plate("1")
            .map(|entry| (entry.display_name.as_str(), entry.notes.as_str())),
        Some(("Ada Byron", "hi"))
    );
}

// ========================================================== pooled teams

#[test]
fn test_commit_pooled_given_a_member_on_another_team_moves_it_to_the_target() {
    let mut roster = roster_of(
        PlateModel::RiderPooled,
        vec![
            entry_team(
                "a",
                "1",
                "Alpha",
                vec![("Ada", "One", Some("1")), ("Bo", "Two", Some("2"))],
            ),
            entry_team(
                "b",
                "3",
                "Beta",
                vec![("Cy", "Three", Some("3")), ("Di", "Four", Some("4"))],
            ),
        ],
    );
    let preview = preview_on(
        &roster,
        &csv("Cy,Three,team,Beta,3,,\nAda,One,team,Beta,1,,\n"),
    );

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 0, 1, 0, 0));
    assert_eq!(actions(&report), vec!["move_rider"]);
    assert_eq!(
        roster
            .entries()
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert_eq!(
        roster
            .resolve_plate("1")
            .map(|entry| (entry.key.as_str(), entry.riders.len())),
        Some(("b", 3))
    );
    assert_eq!(
        roster.entry_by_key("a").map(|entry| entry.riders.len()),
        Some(1)
    );
}

#[test]
fn test_commit_pooled_given_a_solo_member_joins_the_target_team() {
    let mut roster = roster_of(
        PlateModel::RiderPooled,
        vec![
            entry_team(
                "a",
                "1",
                "Wolves",
                vec![("Ada", "One", Some("1")), ("Bo", "Two", Some("2"))],
            ),
            entry_solo("s", "5", ("Cy", "Three"), Some("5")),
        ],
    );
    let preview = preview_on(
        &roster,
        &csv("Ada,One,team,Wolves,1,,\nCy,Three,team,Wolves,5,,\n"),
    );

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 0, 0, 0, 1));
    assert_eq!(actions(&report), vec!["delete_entry", "add_rider_to_team"]);
    assert_eq!(
        roster
            .resolve_plate("1")
            .map(|entry| (entry.display_name.as_str(), entry.riders.len())),
        Some(("Wolves", 3))
    );
}

#[test]
fn test_commit_pooled_given_a_new_member_joins_the_target_team() {
    let mut roster = roster_of(
        PlateModel::RiderPooled,
        vec![entry_team(
            "a",
            "1",
            "Wolves",
            vec![("Ada", "One", Some("1")), ("Bo", "Two", Some("2"))],
        )],
    );
    let preview = preview_on(
        &roster,
        &csv("Ada,One,team,Wolves,1,,\nBo,Two,team,Wolves,2,,\nFi,Six,team,Wolves,7,,\n"),
    );

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 0, 0, 0, 1));
    assert_eq!(actions(&report), vec!["add_rider_to_team"]);
    assert_eq!(
        roster.resolve_plate("1").map(|entry| entry.riders.len()),
        Some(3)
    );
}

#[test]
fn test_commit_pooled_given_solo_members_forms_a_new_team() {
    let mut roster = roster_of(
        PlateModel::RiderPooled,
        vec![
            entry_solo("s1", "1", ("Ada", "One"), Some("1")),
            entry_solo("s2", "2", ("Bo", "Two"), Some("2")),
        ],
    );
    let preview = preview_on(
        &roster,
        &csv("Ada,One,team,Wolves,1,,\nBo,Two,team,Wolves,2,,\n"),
    );

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (1, 0, 0, 0, 0));
    assert_eq!(
        actions(&report),
        vec!["delete_entry", "delete_entry", "create_team_entry"]
    );
    assert_eq!(
        views(&roster)
            .iter()
            .map(|entry| (
                entry.display_name.as_str(),
                entry.plate.as_str(),
                entry.riders.len()
            ))
            .collect::<Vec<_>>(),
        vec![("Wolves", "1", 2)]
    );
}

#[test]
fn test_commit_pooled_given_a_lone_solo_member_forms_a_team_of_one() {
    let mut roster = roster_of(
        PlateModel::RiderPooled,
        vec![entry_solo("s", "1", ("Ada", "One"), Some("1"))],
    );
    let preview = preview_on(&roster, &csv("Ada,One,team,Wolves,1,hi,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (1, 0, 0, 0, 0));
    assert_eq!(
        actions(&report),
        vec!["delete_entry", "create_team_entry_of_one", "update_entry"]
    );
    assert_eq!(
        roster.resolve_plate("1").map(|entry| (
            entry.kind,
            entry.riders.len(),
            entry.notes.as_str()
        )),
        Some((EntryType::Team, 1, "hi"))
    );
}

#[test]
fn test_commit_pooled_given_a_target_team_renames_it_and_updates_the_notes() {
    let mut roster = roster_of(
        PlateModel::RiderPooled,
        vec![entry_team(
            "a",
            "1",
            "Alpha",
            vec![("Ada", "One", Some("1")), ("Bo", "Two", Some("2"))],
        )],
    );
    let preview = preview_on(
        &roster,
        &csv("Ada,One,team,Beta,1,hi,\nBo,Two,team,Beta,2,,\n"),
    );

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (0, 1, 0, 0, 0));
    assert_eq!(actions(&report), vec!["update_entry"]);
    assert_eq!(
        roster
            .resolve_plate("1")
            .map(|entry| (entry.display_name.as_str(), entry.notes.as_str())),
        Some(("Beta", "hi"))
    );
}

#[test]
fn test_commit_pooled_given_the_clean_pooled_fixture_inserts_and_reimports_as_a_no_op() {
    let mut roster = roster_of(PlateModel::RiderPooled, Vec::new());
    let text = include_str!("../../fixtures/clean_pooled.csv");
    let preview = preview_on(&roster, text);

    let report = commit(&mut roster, &preview).expect("the fixture commits");

    assert_eq!(counts(&report), (6, 0, 0, 0, 0));
    assert_eq!(
        roster
            .entries()
            .iter()
            .map(|entry| (
                entry.display_name.as_str(),
                entry.plate.as_str(),
                entry.riders.len()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("Alex Ferreira", "1", 1),
            ("Bo Lindqvist", "2", 1),
            ("Cy Osei", "3", 1),
            ("Do Marchetti", "4", 1),
            ("Falcons", "10", 3),
            ("Hawks", "20", 2),
        ]
    );

    let again = preview_on(&roster, text);
    let second = commit(&mut roster, &again).expect("the re-import commits");

    assert_eq!(counts(&second), (0, 0, 0, 0, 0));
    assert_eq!(second.audit_events, Vec::new());
}

// ============================================================== refusals

#[test]
fn test_commit_given_conflicts_refuses_before_staging() {
    let mut roster = roster_of(PlateModel::TeamRelay, Vec::new());
    let preview = preview_on(
        &roster,
        &csv("Ada,One,solo,,7,,\n,,solo,,8,,\n,,solo,,9,,\n"),
    );

    let error = commit(&mut roster, &preview).expect_err("conflicts are refused");

    assert!(matches!(error, CsvIoError::ConflictsPresent { count: 2 }));
    assert_eq!(roster.entries().len(), 0);
    assert_eq!(roster.audit_log().len(), 0);
}

#[test]
fn test_commit_given_a_stale_preview_refuses() {
    let mut roster = roster_of(PlateModel::TeamRelay, Vec::new());
    let preview = preview_on(&roster, &csv("Ada,Lovelace,solo,,7,,\n"));
    roster
        .create_solo_entry(rider("Zed", "Zulu", None), "9")
        .expect("the plate is free");

    let error = commit(&mut roster, &preview).expect_err("the stale preview is refused");

    assert!(matches!(error, CsvIoError::StalePreview));
    assert_eq!(roster.entries().len(), 1);
    assert_eq!(roster.audit_log().len(), 1);
}

#[test]
fn test_commit_given_a_mid_commit_refusal_leaves_the_roster_unchanged() {
    let mut roster = roster_of(
        PlateModel::RiderPooled,
        vec![entry_team(
            "a",
            "1",
            "Alpha",
            vec![
                ("Ada", "One", Some("1")),
                ("Bo", "Two", Some("2")),
                ("Cy", "Three", Some("3")),
            ],
        )],
    );
    let preview = preview_on(
        &roster,
        &csv("Fi,Six,team,Other,9,,\nAda,One,team,Other,1,,\nDi,Four,team,Other,10,,\n"),
    );
    let before = snapshot(&roster);

    let error = commit(&mut roster, &preview).expect_err("the team ceiling refuses the add");

    assert_eq!(preview.conflicts, Vec::new());
    assert!(matches!(
        error,
        CsvIoError::Roster(RosterError::TeamSize { .. })
    ));
    assert_eq!(snapshot(&roster), before);
}

#[test]
fn test_commit_given_a_full_team_and_a_new_rider_refuses_and_leaves_the_roster_unchanged() {
    let mut roster = roster_of(
        PlateModel::RiderPooled,
        vec![entry_team(
            "a",
            "1",
            "Alpha",
            vec![
                ("Ada", "One", Some("1")),
                ("Bo", "Two", Some("2")),
                ("Cy", "Three", Some("3")),
                ("Di", "Four", Some("4")),
            ],
        )],
    );
    let preview = preview_on(
        &roster,
        &csv("Ada,One,team,Alpha,1,,\nFi,Six,team,Alpha,9,,\n"),
    );
    let before = snapshot(&roster);

    let error = commit(&mut roster, &preview).expect_err("the full team refuses the add");

    assert_eq!(preview.conflicts, Vec::new());
    assert!(matches!(
        error,
        CsvIoError::Roster(RosterError::TeamSize { .. })
    ));
    assert_eq!(snapshot(&roster), before);
}

#[test]
fn test_commit_given_a_header_only_file_changes_nothing() {
    let mut roster = roster_of(PlateModel::TeamRelay, Vec::new());
    let preview = preview_on(&roster, &csv(""));
    let before = snapshot(&roster);

    let report = commit(&mut roster, &preview).expect("the empty preview commits");

    assert_eq!(counts(&report), (0, 0, 0, 0, 0));
    assert_eq!(report.audit_events, Vec::new());
    assert_eq!(snapshot(&roster), before);
}

// ======================================================= audit and survivors

#[test]
fn test_commit_given_earlier_audit_events_reports_only_the_new_ones() {
    let mut roster = roster_of(PlateModel::TeamRelay, Vec::new());
    roster
        .create_solo_entry(rider("Zed", "Zulu", None), "9")
        .expect("the plate is free");
    let preview = preview_on(&roster, &csv("Ada,Lovelace,solo,,7,,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(report.audit_events.len(), 1);
    assert_eq!(
        report.audit_events,
        roster
            .audit_log()
            .iter()
            .skip(1)
            .cloned()
            .collect::<Vec<_>>()
    );
    assert_eq!(actions(&report), vec!["create_solo_entry"]);
}

#[rstest]
#[case(PlateModel::TeamRelay)]
#[case(PlateModel::RiderPooled)]
fn test_commit_given_an_entry_absent_from_the_file_keeps_it(#[case] model: PlateModel) {
    let mut roster = roster_of(
        model,
        vec![entry_solo(
            "keep",
            "9",
            ("Zed", "Zulu"),
            own_plate(model, "9"),
        )],
    );
    let preview = preview_on(&roster, &csv("Ada,Lovelace,solo,,7,,\n"));

    let report = commit(&mut roster, &preview).expect("the preview commits");

    assert_eq!(counts(&report), (1, 0, 0, 0, 0));
    assert_eq!(roster.entries().len(), 2);
    assert_eq!(
        roster
            .resolve_plate("9")
            .map(|entry| entry.display_name.as_str()),
        Some("Zed Zulu")
    );
}
