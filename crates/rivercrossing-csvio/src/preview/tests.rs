// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the pure import preview.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use pretty_assertions::assert_eq;
use rivercrossing_roster::enums::{EntryMode, EntryType, PlateModel, RideStatus};
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::{Entry, Rider};
use rstest::rstest;

use super::{compare_plates, next_plate, preview, preview_bytes};
use crate::error::CsvIoError;
use crate::model::{
    ImportOptions, ImportPreview, ImportProblem, ParsedEntry, ParsedRider, PreviewFingerprint,
    ProblemCode,
};

/// The source path that every inline test previews under.
const SOURCE: &str = "entries.csv";

/// The canonical export header, in the column order the tests rely on.
const HEADER: &str = "FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX\n";

/// The header problem text.
const HEADER_PROBLEM: &str = "missing or malformed header: no first or last name column";

/// The under-min team warning text for one rider.
const UNDER_MIN_ONE: &str = "team of 1 rider is below the minimum of 2 (team-under-min)";

// ---------------------------------------------------------------- helpers

/// Return *rows* under the canonical header.
fn csv(rows: &str) -> String {
    format!("{HEADER}{rows}")
}

fn options() -> ImportOptions {
    ImportOptions::default()
}

fn convert_only() -> ImportOptions {
    ImportOptions {
        map_unknown_sex_to_male: false,
        convert_teams_of_one_to_solo: true,
    }
}

fn map_sex_only() -> ImportOptions {
    ImportOptions {
        map_unknown_sex_to_male: true,
        convert_teams_of_one_to_solo: false,
    }
}

fn map_sex_and_convert() -> ImportOptions {
    ImportOptions {
        map_unknown_sex_to_male: true,
        convert_teams_of_one_to_solo: true,
    }
}

fn mixed(model: PlateModel) -> Roster {
    Roster::new(EntryMode::Mixed, 4, model, None).expect("the settings are valid")
}

fn relay_roster() -> Roster {
    mixed(PlateModel::TeamRelay)
}

fn pooled_roster() -> Roster {
    mixed(PlateModel::RiderPooled)
}

fn roster_of(model: PlateModel, status: RideStatus, entries: Vec<Entry>) -> Roster {
    let mut roster = mixed(model);
    roster.set_status(status);
    roster.load_entries(entries);
    roster
}

fn rider(first: &str, last: &str, plate: Option<&str>) -> Rider {
    Rider {
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        plate: plate.map(str::to_owned),
        ..Rider::default()
    }
}

fn team_entry(key: &str, plate: &str, riders: Vec<Rider>) -> Entry {
    Entry::restored(key, plate, "Team", EntryType::Team, riders)
}

fn solo_member(key: &str, plate: &str, name: (&str, &str)) -> Entry {
    let (first, last) = name;
    Entry::restored(
        key,
        plate,
        format!("{first} {last}"),
        EntryType::Solo,
        vec![rider(first, last, Some(plate))],
    )
}

fn run(text: &str) -> ImportPreview {
    run_with(&relay_roster(), text, options())
}

fn run_on(roster: &Roster, text: &str) -> ImportPreview {
    run_with(roster, text, options())
}

fn run_with(roster: &Roster, text: &str, options: ImportOptions) -> ImportPreview {
    preview_bytes(Path::new(SOURCE), text.as_bytes(), roster, options)
        .expect("the payload previews")
}

fn fixture(name: &str, text: &str, roster: &Roster) -> ImportPreview {
    preview_bytes(Path::new(name), text.as_bytes(), roster, options())
        .expect("the fixture previews")
}

fn counts(preview: &ImportPreview) -> (usize, usize, usize, usize, usize) {
    (
        preview.riders,
        preview.teams,
        preview.entries.len(),
        preview.conflicts.len(),
        preview.warnings.len(),
    )
}

fn problem_rows(problems: &[ImportProblem]) -> Vec<usize> {
    problems.iter().map(|problem| problem.row).collect()
}

fn problem_codes(problems: &[ImportProblem]) -> Vec<ProblemCode> {
    problems
        .iter()
        .map(|problem| problem.code.clone())
        .collect()
}

fn conflict(row: usize, code: ProblemCode, message: &str) -> ImportProblem {
    ImportProblem {
        row,
        code,
        message: message.to_owned(),
    }
}

fn plates(preview: &ImportPreview) -> Vec<&str> {
    preview
        .entries
        .iter()
        .map(|entry| entry.plate.as_str())
        .collect()
}

// ========================================================== whole-file rows

#[test]
fn test_preview_given_an_empty_file_returns_one_header_conflict_at_row_one() {
    let preview = run("");

    assert_eq!(counts(&preview), (0, 0, 0, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(1, ProblemCode::Header, HEADER_PROBLEM)]
    );
    assert_eq!(preview.source_path, PathBuf::from(SOURCE));
}

#[test]
fn test_preview_given_no_name_column_returns_one_header_conflict_at_row_one() {
    let preview = run("TEAMNAME,PLATE\nAlpha,1\n");

    assert_eq!(counts(&preview), (0, 0, 0, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(1, ProblemCode::Header, HEADER_PROBLEM)]
    );
}

#[test]
fn test_preview_given_a_header_and_no_data_rows_returns_an_empty_preview() {
    let preview = run(&csv(""));

    assert_eq!(counts(&preview), (0, 0, 0, 0, 0));
}

#[test]
fn test_preview_given_invalid_utf8_returns_not_utf8() {
    let error = preview_bytes(
        Path::new(SOURCE),
        b"FIRSTNAME,LASTNAME\n\xFF,Lovelace\n",
        &relay_roster(),
        options(),
    )
    .expect_err("invalid utf-8 is refused");

    assert!(matches!(error, CsvIoError::NotUtf8));
}

#[test]
fn test_preview_given_a_missing_column_reads_it_as_blank() {
    let preview = run("FIRSTNAME,PLATE\nAda,7\n");

    assert_eq!(
        preview.entries,
        vec![ParsedEntry {
            plate: "7".to_owned(),
            display_name: "Ada".to_owned(),
            kind: EntryType::Solo,
            riders: vec![ParsedRider {
                first_name: "Ada".to_owned(),
                last_name: String::new(),
                plate: None,
                sex: None,
            }],
            notes: String::new(),
        }]
    );
}

// ============================================================ row parsing

#[test]
fn test_preview_given_a_footer_row_without_a_name_or_plate_skips_it() {
    let preview = run(&csv(",,,,,,\n"));

    assert_eq!(counts(&preview), (0, 0, 0, 0, 0));
}

#[test]
fn test_preview_given_a_plated_nameless_row_returns_missing_name() {
    let preview = run(&csv(",,solo,,7,,\n"));

    assert_eq!(counts(&preview), (0, 0, 0, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(2, ProblemCode::MissingName, "missing name")]
    );
}

#[test]
fn test_preview_given_a_team_nameless_row_returns_missing_name() {
    let preview = run(&csv(",,team,Alpha,,,\n"));

    assert_eq!(
        preview.conflicts,
        vec![conflict(2, ProblemCode::MissingName, "missing name")]
    );
}

#[test]
fn test_preview_given_an_invalid_sex_returns_an_invalid_sex_conflict() {
    let preview = run(&csv("Ada,Lovelace,solo,,7,,X\n"));

    assert_eq!(counts(&preview), (0, 0, 0, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(2, ProblemCode::InvalidSex, "invalid sex \"X\"")]
    );
}

#[test]
fn test_preview_given_an_invalid_sex_and_the_opt_in_maps_it_to_male() {
    let preview = run_with(
        &relay_roster(),
        &csv("Ada,Lovelace,solo,,7,,X\n"),
        map_sex_only(),
    );

    assert_eq!(counts(&preview), (1, 0, 1, 0, 0));
    assert_eq!(preview.entries[0].riders[0].sex.as_deref(), Some("M"));
}

#[test]
fn test_preview_given_a_blank_sex_without_the_opt_in_leaves_it_unknown() {
    let preview = run(&csv("Ada,Lovelace,solo,,7,,\n"));

    assert_eq!(preview.entries[0].riders[0].sex, None);
}

#[test]
fn test_preview_given_a_blank_sex_and_the_opt_in_maps_it_to_male() {
    let preview = run_with(
        &relay_roster(),
        &csv("Ada,Lovelace,solo,,7,,\n"),
        map_sex_only(),
    );

    assert_eq!(preview.entries[0].riders[0].sex.as_deref(), Some("M"));
}

#[test]
fn test_preview_given_an_unknown_type_returns_an_unknown_type_conflict() {
    let preview = run(&csv("Ada,Lovelace,duo,,7,,\n"));

    assert_eq!(counts(&preview), (0, 0, 0, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            2,
            ProblemCode::UnknownType,
            "unknown entry type \"duo\""
        )]
    );
}

#[test]
fn test_preview_given_an_explicit_team_without_a_team_name_returns_missing_name() {
    let preview = run(&csv("Ada,Lovelace,team,,,,\n"));

    assert_eq!(
        preview.conflicts,
        vec![conflict(2, ProblemCode::MissingName, "missing name")]
    );
}

#[test]
fn test_preview_given_a_blank_type_and_a_team_name_derives_team() {
    let preview = run(&csv("Ada,Lovelace,,Wolves,,,\n"));

    assert_eq!(counts(&preview), (1, 1, 1, 0, 1));
    assert_eq!(preview.entries[0].kind, EntryType::Team);
    assert_eq!(preview.entries[0].display_name, "Wolves");
}

#[test]
fn test_preview_given_a_one_word_rider_returns_the_first_name_as_display_name() {
    let preview = run(&csv("Cher,,solo,,7,,\n"));

    assert_eq!(preview.entries[0].display_name, "Cher");
}

#[test]
fn test_preview_given_a_last_name_only_rider_returns_the_last_name_as_display_name() {
    let preview = run(&csv(",Lovelace,solo,,7,,\n"));

    assert_eq!(preview.entries[0].display_name, "Lovelace");
}

#[test]
fn test_preview_given_a_short_row_reads_the_missing_cells_as_blank() {
    let preview = run(&csv("Ada\n"));

    assert_eq!(counts(&preview), (1, 0, 1, 0, 0));
    assert_eq!(preview.entries[0].display_name, "Ada");
    assert_eq!(preview.entries[0].plate, "1");
}

#[test]
fn test_preview_given_uniform_case_names_canonicalises_them() {
    let preview = run(&csv("ADA,LOVELACE,solo,,7,,\n"));

    assert_eq!(preview.entries[0].display_name, "Ada Lovelace");
}

#[test]
fn test_preview_given_a_mixed_case_surname_preserves_it() {
    let preview = run(&csv("Ronald,McDonald,solo,,7,,\n"));

    assert_eq!(preview.entries[0].display_name, "Ronald McDonald");
}

// ============================================================= solo-only

#[test]
fn test_preview_given_a_team_row_on_a_solo_only_ride_returns_solo_only_team() {
    let roster = Roster::new(EntryMode::Solo, 4, PlateModel::TeamRelay, None)
        .expect("the settings are valid");
    let preview = run_on(
        &roster,
        &csv("Ada,One,team,Wolves,,,\nBo,Two,team,Wolves,,,\nCy,Three,solo,,7,,\n"),
    );

    assert_eq!(counts(&preview), (1, 0, 1, 2, 0));
    assert_eq!(
        preview.conflicts,
        vec![
            conflict(
                2,
                ProblemCode::SoloOnlyTeam,
                "team entries are not allowed on a solo-only ride"
            ),
            conflict(
                3,
                ProblemCode::SoloOnlyTeam,
                "team entries are not allowed on a solo-only ride"
            ),
        ]
    );
}

// ================================================================ plates

#[test]
fn test_preview_given_duplicate_plates_returns_one_conflict_and_keeps_both_entries() {
    let preview = run(&csv("Ada,One,solo,,1,,\nBo,Two,solo,,1,,\n"));

    assert_eq!(counts(&preview), (2, 0, 2, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            3,
            ProblemCode::DuplicatePlate,
            "duplicate plate 1"
        )]
    );
}

#[test]
fn test_preview_given_a_non_digit_plate_returns_non_digit_plate() {
    let preview = run_on(
        &pooled_roster(),
        &csv("Ada,One,solo,,77A,,\nBo,Two,solo,,2,,\n"),
    );

    assert_eq!(counts(&preview), (1, 0, 1, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            2,
            ProblemCode::NonDigitPlate,
            "plate \"77A\" must be a whole number"
        )]
    );
}

#[test]
fn test_preview_relay_given_a_non_digit_solo_plate_returns_non_digit_plate() {
    let preview = run(&csv("Ada,One,solo,,77A,,\n"));

    assert_eq!(counts(&preview), (0, 0, 0, 1, 0));
    assert_eq!(preview.conflicts[0].code, ProblemCode::NonDigitPlate);
}

#[test]
fn test_preview_relay_given_a_non_digit_team_plate_returns_non_digit_plate() {
    let preview = run(&csv("Ada,One,team,Wolves,7A,,\nBo,Two,team,Wolves,7A,,\n"));

    assert_eq!(counts(&preview), (0, 0, 0, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            2,
            ProblemCode::NonDigitPlate,
            "plate \"7A\" must be a whole number"
        )]
    );
}

#[test]
fn test_preview_given_a_blank_plate_allocates_the_next_free_number() {
    let preview = run(&csv("Ada,One,solo,,,,,\n"));

    assert_eq!(plates(&preview), vec!["1"]);
}

#[test]
fn test_preview_given_a_blank_plate_skips_a_file_named_plate() {
    let preview = run(&csv("Ada,One,solo,,3,,\nBo,Two,solo,,,,,\n"));

    assert_eq!(plates(&preview), vec!["3", "4"]);
}

#[test]
fn test_preview_given_a_blank_plate_skips_roster_and_rider_plates() {
    let roster = roster_of(
        PlateModel::RiderPooled,
        RideStatus::Draft,
        vec![
            solo_member("solo-1", "1", ("Cy", "Three")),
            team_entry(
                "team-a",
                "4",
                vec![
                    rider("Ada", "One", Some("4")),
                    rider("Bo", "Two", Some("5")),
                ],
            ),
        ],
    );

    let preview = run_on(&roster, &csv("Di,Four,solo,,,,,\n"));

    assert_eq!(plates(&preview), vec!["6"]);
}

#[test]
fn test_preview_relay_given_a_blank_team_plate_allocates_one_shared_plate() {
    let preview = run(&csv("Ada,One,team,Wolves,,,\nBo,Two,team,Wolves,,,\n"));

    assert_eq!(counts(&preview), (2, 1, 1, 0, 0));
    assert_eq!(preview.entries[0].plate, "1");
    assert_eq!(
        preview.entries[0]
            .riders
            .iter()
            .map(|rider| rider.plate.clone())
            .collect::<Vec<_>>(),
        vec![None, None]
    );
}

#[test]
fn test_preview_pooled_given_a_team_adopts_the_numeric_lowest_member_plate() {
    let preview = run_on(
        &pooled_roster(),
        &csv("El,Novak,team,Falcons,10,,\nFa,Rahman,team,Falcons,9,,\n"),
    );

    assert_eq!(counts(&preview), (2, 1, 1, 0, 0));
    assert_eq!(preview.entries[0].plate, "9");
    assert_eq!(
        preview.entries[0]
            .riders
            .iter()
            .map(|rider| rider.plate.clone())
            .collect::<Vec<_>>(),
        vec![Some("10".to_owned()), Some("9".to_owned())]
    );
}

#[test]
fn test_preview_relay_given_a_team_uses_its_single_shared_plate() {
    let preview = run(&csv("Ada,One,team,Wolves,5,,\nBo,Two,team,Wolves,5,,\n"));

    assert_eq!(preview.entries[0].plate, "5");
}

#[test]
fn test_preview_relay_given_a_team_naming_two_plates_returns_team_rows_different_plates() {
    let preview = run(&csv("Ada,One,team,Wolves,5,,\nBo,Two,team,Wolves,6,,\n"));

    assert_eq!(counts(&preview), (0, 0, 0, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            2,
            ProblemCode::TeamRowsDifferentPlates,
            "team rows carry different plates (5, 6)"
        )]
    );
}

#[test]
fn test_preview_relay_given_a_team_naming_three_plates_sorts_them_in_the_message() {
    let preview = run(&csv(
        "Ada,One,team,Wolves,7,,\nBo,Two,team,Wolves,10,,\nCy,Three,team,Wolves,9,,\n",
    ));

    assert_eq!(
        preview.conflicts[0].message,
        "team rows carry different plates (10, 7, 9)"
    );
}

// ============================================================= team sizes

#[test]
fn test_preview_relay_given_a_team_of_one_in_draft_returns_an_under_min_warning() {
    let preview = run(&csv("Ada,One,team,Wolves,5,,\n"));

    assert_eq!(counts(&preview), (1, 1, 1, 0, 1));
    assert_eq!(
        preview.warnings,
        vec![conflict(2, ProblemCode::TeamUnderMin, UNDER_MIN_ONE)]
    );
}

#[test]
fn test_preview_relay_given_a_team_of_one_on_a_started_ride_returns_an_under_min_conflict() {
    let roster = roster_of(PlateModel::TeamRelay, RideStatus::Running, Vec::new());

    let preview = run_on(&roster, &csv("Ada,One,team,Wolves,5,,\n"));

    assert_eq!(counts(&preview), (1, 1, 1, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(2, ProblemCode::TeamUnderMin, UNDER_MIN_ONE)]
    );
}

#[test]
fn test_preview_pooled_given_a_team_of_one_in_draft_returns_an_under_min_warning() {
    let preview = run_on(&pooled_roster(), &csv("Ada,One,team,Wolves,5,,\n"));

    assert_eq!(counts(&preview), (1, 1, 1, 0, 1));
    assert_eq!(preview.warnings[0].code, ProblemCode::TeamUnderMin);
}

#[test]
fn test_preview_pooled_given_a_team_of_one_on_a_started_ride_returns_an_under_min_conflict() {
    let roster = roster_of(PlateModel::RiderPooled, RideStatus::Running, Vec::new());

    let preview = run_on(&roster, &csv("Ada,One,team,Wolves,5,,\n"));

    assert_eq!(counts(&preview), (1, 1, 1, 1, 0));
    assert_eq!(preview.conflicts[0].code, ProblemCode::TeamUnderMin);
}

#[test]
fn test_preview_relay_given_a_team_over_the_maximum_returns_team_over_max() {
    let mut roster = mixed(PlateModel::TeamRelay);
    roster
        .update_shape(EntryMode::Mixed, 2, PlateModel::TeamRelay)
        .expect("two is a valid team size");

    let preview = run_on(
        &roster,
        &csv("Ada,One,team,Wolves,,,\nBo,Two,team,Wolves,,,\nCy,Three,team,Wolves,,,\n"),
    );

    assert_eq!(counts(&preview), (3, 1, 1, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            2,
            ProblemCode::TeamOverMax,
            "team of 3 riders exceeds the maximum of 2 (team-over-max)"
        )]
    );
}

#[test]
fn test_preview_pooled_given_a_team_over_the_maximum_returns_team_over_max() {
    let mut roster = mixed(PlateModel::RiderPooled);
    roster
        .update_shape(EntryMode::Mixed, 2, PlateModel::RiderPooled)
        .expect("two is a valid team size");

    let preview = run_on(
        &roster,
        &csv("Ada,One,team,Wolves,1,,\nBo,Two,team,Wolves,2,,\nCy,Three,team,Wolves,3,,\n"),
    );

    assert_eq!(counts(&preview), (3, 1, 1, 1, 0));
    assert_eq!(preview.conflicts[0].code, ProblemCode::TeamOverMax);
}

#[test]
fn test_preview_relay_given_a_team_at_the_maximum_returns_no_conflict() {
    let mut roster = mixed(PlateModel::TeamRelay);
    roster
        .update_shape(EntryMode::Mixed, 2, PlateModel::TeamRelay)
        .expect("two is a valid team size");

    let preview = run_on(
        &roster,
        &csv("Ada,One,team,Wolves,,,\nBo,Two,team,Wolves,,,\n"),
    );

    assert_eq!(counts(&preview), (2, 1, 1, 0, 0));
}

// ======================================================= relay reshaping

#[test]
fn test_preview_relay_given_a_changed_team_composition_on_a_started_ride_returns_blocked_reshape() {
    let roster = roster_of(
        PlateModel::TeamRelay,
        RideStatus::Running,
        vec![team_entry(
            "team-a",
            "5",
            vec![
                rider("Ada", "Lovelace", None),
                rider("Alan", "Turing", None),
            ],
        )],
    );

    let preview = run_on(
        &roster,
        &csv("Ada,Lovelace,team,Wolves,5,,\nGrace,Hopper,team,Wolves,5,,\n"),
    );

    assert_eq!(counts(&preview), (2, 1, 1, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            2,
            ProblemCode::BlockedReshape,
            "team composition changed but the ride is running; only new plates or name fixes are allowed"
        )]
    );
}

#[test]
fn test_preview_relay_given_a_changed_team_composition_in_draft_returns_no_conflict() {
    let roster = roster_of(
        PlateModel::TeamRelay,
        RideStatus::Draft,
        vec![team_entry(
            "team-a",
            "5",
            vec![
                rider("Ada", "Lovelace", None),
                rider("Alan", "Turing", None),
            ],
        )],
    );

    let preview = run_on(
        &roster,
        &csv("Ada,Lovelace,team,Wolves,5,,\nGrace,Hopper,team,Wolves,5,,\n"),
    );

    assert_eq!(counts(&preview), (2, 1, 1, 0, 0));
}

#[test]
fn test_preview_relay_given_an_unchanged_team_on_a_started_ride_returns_no_conflict() {
    let roster = roster_of(
        PlateModel::TeamRelay,
        RideStatus::Running,
        vec![team_entry(
            "team-a",
            "5",
            vec![
                rider("Ada", "Lovelace", None),
                rider("Alan", "Turing", None),
            ],
        )],
    );

    let preview = run_on(
        &roster,
        &csv("Ada,Lovelace,team,Wolves,5,,\nAlan,Turing,team,Wolves,5,,\n"),
    );

    assert_eq!(counts(&preview), (2, 1, 1, 0, 0));
}

#[test]
fn test_preview_relay_given_a_solo_row_matching_a_team_plate_on_a_started_ride_returns_blocked_reshape()
 {
    let roster = roster_of(
        PlateModel::TeamRelay,
        RideStatus::Running,
        vec![team_entry(
            "team-a",
            "5",
            vec![rider("Ada", "Lovelace", None)],
        )],
    );

    let preview = run_on(&roster, &csv("Ada,Lovelace,solo,,5,,\n"));

    assert_eq!(counts(&preview), (1, 0, 1, 1, 0));
    assert_eq!(preview.conflicts[0].code, ProblemCode::BlockedReshape);
}

#[test]
fn test_preview_relay_given_a_solo_row_matching_a_solo_plate_on_a_started_ride_returns_no_conflict()
{
    let roster = roster_of(
        PlateModel::TeamRelay,
        RideStatus::Running,
        vec![solo_member("solo-1", "5", ("Ada", "Lovelace"))],
    );

    let preview = run_on(&roster, &csv("Ada,Lovelace,solo,,5,,\n"));

    assert_eq!(counts(&preview), (1, 0, 1, 0, 0));
}

// ======================================================= entry order and names

#[test]
fn test_preview_relay_given_interleaved_team_rows_anchors_the_team_at_its_first_row() {
    let preview = run(&csv(
        "Ada,One,team,A,1,,\nBo,Two,solo,,2,,\nCy,Three,team,A,1,,\nDi,Four,team,B,3,,\nEd,Five,team,B,3,,\n",
    ));

    assert_eq!(
        preview
            .entries
            .iter()
            .map(|entry| entry.display_name.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "Bo Two", "B"]
    );
}

#[test]
fn test_preview_relay_given_two_spellings_of_one_team_merges_them() {
    let preview = run(&csv(
        "Ada,One,team,Moss Ridge   Riders,,,\nBo,Two,team,moss ridge riders,,,\n",
    ));

    assert_eq!(counts(&preview), (2, 1, 1, 0, 0));
    assert_eq!(preview.entries[0].display_name, "Moss Ridge Riders");
}

#[test]
fn test_preview_relay_given_team_rows_with_notes_joins_them() {
    let preview = run(&csv(
        "Ada,One,team,Wolves,,first,\nBo,Two,team,Wolves,,,\nCy,Three,team,Wolves,,second,\n",
    ));

    assert_eq!(preview.entries[0].notes, "first; second");
}

#[test]
fn test_preview_pooled_orders_solo_entries_before_team_entries() {
    let preview = run_on(
        &pooled_roster(),
        &csv("Ada,One,team,Wolves,10,,\nBo,Two,team,Wolves,9,,\nCy,Three,solo,,1,,\n"),
    );

    assert_eq!(
        preview
            .entries
            .iter()
            .map(|entry| entry.display_name.as_str())
            .collect::<Vec<_>>(),
        vec!["Cy Three", "Wolves"]
    );
}

#[test]
fn test_preview_pooled_given_two_spellings_of_one_team_keeps_the_first_seen_name() {
    let preview = run_on(
        &pooled_roster(),
        &csv("Ada,One,team,Loop  Legends,1,,\nBo,Two,team,loop  legends,2,,\n"),
    );

    assert_eq!(preview.entries[0].display_name, "Loop Legends");
}

// ===================================================== convert-teams opt-in

#[test]
fn test_preview_relay_given_a_team_of_one_and_the_convert_opt_in_in_draft_returns_a_solo() {
    let preview = run_with(
        &relay_roster(),
        &csv("Ada,One,team,Wolves,5,,\n"),
        convert_only(),
    );

    assert_eq!(counts(&preview), (1, 0, 1, 0, 0));
    assert_eq!(preview.entries[0].kind, EntryType::Solo);
    assert_eq!(preview.entries[0].plate, "5");
    assert_eq!(preview.entries[0].riders[0].plate, None);
}

#[test]
fn test_preview_pooled_given_a_team_of_one_and_the_convert_opt_in_in_draft_returns_a_solo() {
    let preview = run_with(
        &pooled_roster(),
        &csv("Ada,One,team,Wolves,5,,\n"),
        convert_only(),
    );

    assert_eq!(counts(&preview), (1, 0, 1, 0, 0));
    assert_eq!(preview.entries[0].kind, EntryType::Solo);
    assert_eq!(preview.entries[0].plate, "5");
    assert_eq!(preview.entries[0].riders[0].plate.as_deref(), Some("5"));
}

#[test]
fn test_preview_relay_given_a_team_of_one_and_the_opt_in_on_a_started_ride_returns_under_min() {
    let roster = roster_of(PlateModel::TeamRelay, RideStatus::Running, Vec::new());

    let preview = run_with(&roster, &csv("Ada,One,team,Wolves,5,,\n"), convert_only());

    assert_eq!(counts(&preview), (1, 1, 1, 1, 0));
    assert_eq!(preview.entries[0].kind, EntryType::Team);
    assert_eq!(preview.conflicts[0].code, ProblemCode::TeamUnderMin);
}

#[test]
fn test_preview_pooled_given_a_team_of_one_and_the_opt_in_on_a_started_ride_returns_under_min() {
    let roster = roster_of(PlateModel::RiderPooled, RideStatus::Running, Vec::new());

    let preview = run_with(&roster, &csv("Ada,One,team,Wolves,5,,\n"), convert_only());

    assert_eq!(counts(&preview), (1, 1, 1, 1, 0));
    assert_eq!(preview.entries[0].kind, EntryType::Team);
    assert_eq!(preview.conflicts[0].code, ProblemCode::TeamUnderMin);
}

#[test]
fn test_preview_relay_given_a_team_of_one_and_the_opt_in_matching_a_team_on_a_started_ride_returns_blocked_reshape()
 {
    let roster = roster_of(
        PlateModel::TeamRelay,
        RideStatus::Running,
        vec![team_entry(
            "team-a",
            "5",
            vec![
                rider("Ada", "Lovelace", None),
                rider("Alan", "Turing", None),
            ],
        )],
    );

    let preview = run_with(
        &roster,
        &csv("Ada,Lovelace,team,Wolves,5,,\n"),
        convert_only(),
    );

    assert_eq!(counts(&preview), (0, 0, 0, 1, 0));
    assert_eq!(preview.conflicts[0].code, ProblemCode::BlockedReshape);
}

#[test]
fn test_preview_pooled_given_a_team_of_one_and_the_opt_in_matching_a_team_on_a_started_ride_returns_blocked_reshape()
 {
    let roster = roster_of(
        PlateModel::RiderPooled,
        RideStatus::Running,
        vec![team_entry(
            "team-a",
            "5",
            vec![
                rider("Ada", "Lovelace", Some("5")),
                rider("Alan", "Turing", Some("6")),
            ],
        )],
    );

    let preview = run_with(
        &roster,
        &csv("Ada,Lovelace,team,Wolves,5,,\n"),
        convert_only(),
    );

    assert_eq!(counts(&preview), (0, 0, 0, 1, 0));
    assert_eq!(
        preview.conflicts[0].message,
        "converting a team member to a solo entry requires DRAFT (ride is running)"
    );
}

#[test]
fn test_preview_relay_given_both_opt_ins_maps_the_sex_and_converts_the_team_of_one() {
    let preview = run_with(
        &relay_roster(),
        &csv("Ada,One,team,Wolves,5,,X\n"),
        map_sex_and_convert(),
    );

    assert_eq!(counts(&preview), (1, 0, 1, 0, 0));
    assert_eq!(preview.entries[0].kind, EntryType::Solo);
    assert_eq!(preview.entries[0].riders[0].sex.as_deref(), Some("M"));
}

#[test]
fn test_preview_pooled_given_both_opt_ins_maps_the_sex_and_converts_the_team_of_one() {
    let preview = run_with(
        &pooled_roster(),
        &csv("Ada,One,team,Wolves,5,,X\n"),
        map_sex_and_convert(),
    );

    assert_eq!(counts(&preview), (1, 0, 1, 0, 0));
    assert_eq!(preview.entries[0].kind, EntryType::Solo);
    assert_eq!(preview.entries[0].riders[0].sex.as_deref(), Some("M"));
}

#[test]
fn test_preview_relay_given_two_riders_and_the_convert_opt_in_keeps_the_team() {
    let preview = run_with(
        &relay_roster(),
        &csv("Ada,One,team,Wolves,5,,\nBo,Two,team,Wolves,5,,\n"),
        convert_only(),
    );

    assert_eq!(counts(&preview), (2, 1, 1, 0, 0));
    assert_eq!(preview.entries[0].kind, EntryType::Team);
}

// ====================================================== pooled reshaping

#[test]
fn test_preview_pooled_given_duplicate_plates_flags_the_later_row_and_keeps_both_entries() {
    let preview = run_on(
        &pooled_roster(),
        &csv("Ada,One,solo,,1,,\nBo,Two,solo,,1,,\n"),
    );

    assert_eq!(counts(&preview), (2, 0, 2, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            3,
            ProblemCode::DuplicatePlate,
            "duplicate plate 1"
        )]
    );
}

#[test]
fn test_preview_pooled_given_a_solo_row_matching_a_roster_solo_on_a_started_ride_returns_no_conflict()
 {
    let roster = roster_of(
        PlateModel::RiderPooled,
        RideStatus::Running,
        vec![solo_member("solo-1", "1", ("Cy", "Three"))],
    );

    let preview = run_on(&roster, &csv("Cy,Three,solo,,1,,\n"));

    assert_eq!(counts(&preview), (1, 0, 1, 0, 0));
}

#[test]
fn test_preview_pooled_given_a_member_owned_by_another_team_on_a_started_ride_returns_blocked_reshape()
 {
    let roster = roster_of(
        PlateModel::RiderPooled,
        RideStatus::Running,
        vec![
            team_entry(
                "team-a",
                "2",
                vec![
                    rider("Cy", "Three", Some("2")),
                    rider("Di", "Four", Some("3")),
                ],
            ),
            team_entry("team-b", "4", vec![rider("Ed", "Five", Some("4"))]),
        ],
    );

    let preview = run_on(
        &roster,
        &csv("Ada,One,team,New,2,,\nBo,Two,team,New,3,,\nEd,Five,team,New,4,,\n"),
    );

    assert_eq!(counts(&preview), (3, 1, 1, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            4,
            ProblemCode::BlockedReshape,
            "a team membership change requires DRAFT (ride is running)"
        )]
    );
}

#[test]
fn test_preview_pooled_given_a_solo_rider_joining_a_team_on_a_started_ride_returns_blocked_reshape()
{
    let roster = roster_of(
        PlateModel::RiderPooled,
        RideStatus::Running,
        vec![solo_member("solo-1", "1", ("Cy", "Three"))],
    );

    let preview = run_on(&roster, &csv("Ada,One,team,New,1,,\nBo,Two,team,New,7,,\n"));

    assert_eq!(
        preview.conflicts,
        vec![conflict(
            2,
            ProblemCode::BlockedReshape,
            "converting a solo rider into a team member requires DRAFT (ride is running)"
        )]
    );
}

#[test]
fn test_preview_pooled_given_a_team_member_turned_solo_on_a_started_ride_returns_blocked_reshape() {
    let roster = roster_of(
        PlateModel::RiderPooled,
        RideStatus::Running,
        vec![team_entry(
            "team-a",
            "2",
            vec![
                rider("Cy", "Three", Some("2")),
                rider("Di", "Four", Some("3")),
            ],
        )],
    );

    let preview = run_on(&roster, &csv("Cy,Three,solo,,2,,\n"));

    assert_eq!(counts(&preview), (1, 0, 1, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            2,
            ProblemCode::BlockedReshape,
            "converting a team member to a solo entry requires DRAFT (ride is running)"
        )]
    );
}

#[test]
fn test_preview_pooled_given_a_new_team_on_a_started_ride_returns_no_conflict() {
    let roster = roster_of(PlateModel::RiderPooled, RideStatus::Running, Vec::new());

    let preview = run_on(
        &roster,
        &csv("Ada,One,team,Wolves,1,,\nBo,Two,team,Wolves,2,,\n"),
    );

    assert_eq!(counts(&preview), (2, 1, 1, 0, 0));
}

#[test]
fn test_preview_pooled_given_a_new_plate_joining_existing_team_on_a_started_ride_returns_blocked_reshape()
 {
    let roster = roster_of(
        PlateModel::RiderPooled,
        RideStatus::Running,
        vec![team_entry(
            "team-a",
            "2",
            vec![
                rider("Cy", "Three", Some("2")),
                rider("Di", "Four", Some("3")),
            ],
        )],
    );

    let preview = run_on(
        &roster,
        &csv("Cy,Three,team,Wolves,2,,\nAda,One,team,Wolves,5,,\n"),
    );

    assert_eq!(counts(&preview), (2, 1, 1, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            3,
            ProblemCode::BlockedReshape,
            "a team membership change requires DRAFT (ride is running)"
        )]
    );
}

#[test]
fn test_preview_pooled_given_an_unchanged_team_on_a_started_ride_returns_no_conflict() {
    let roster = roster_of(
        PlateModel::RiderPooled,
        RideStatus::Running,
        vec![team_entry(
            "team-a",
            "2",
            vec![
                rider("Cy", "Three", Some("2")),
                rider("Di", "Four", Some("3")),
            ],
        )],
    );

    let preview = run_on(
        &roster,
        &csv("Cy,Three,team,Wolves,2,,\nDi,Four,team,Wolves,3,,\n"),
    );

    assert_eq!(counts(&preview), (2, 1, 1, 0, 0));
}

#[test]
fn test_preview_pooled_given_the_same_reshapes_in_draft_returns_no_conflicts() {
    let roster = roster_of(
        PlateModel::RiderPooled,
        RideStatus::Draft,
        vec![
            solo_member("solo-1", "1", ("Cy", "Three")),
            team_entry(
                "team-a",
                "2",
                vec![
                    rider("Di", "Four", Some("2")),
                    rider("Ed", "Five", Some("3")),
                ],
            ),
        ],
    );

    let preview = run_on(
        &roster,
        &csv("Ada,One,solo,,2,,\nBo,Two,team,New,1,,\nFi,Six,team,New,9,,\n"),
    );

    assert_eq!(counts(&preview), (3, 1, 2, 0, 0));
}

// ================================================================ warnings

#[test]
fn test_preview_given_a_repeated_rider_name_returns_one_warning_at_the_second_row() {
    let preview = run(&csv("Ada,One,solo,,1,,\nada,one,solo,,2,,\n"));

    assert_eq!(counts(&preview), (2, 0, 2, 0, 1));
    assert_eq!(
        preview.warnings,
        vec![conflict(
            3,
            ProblemCode::DuplicateRiderName,
            "duplicate rider name Ada One"
        )]
    );
}

#[test]
fn test_preview_given_a_third_repeat_of_a_rider_name_adds_no_warning() {
    let preview = run(&csv(
        "Ada,One,solo,,1,,\nada,one,solo,,2,,\nADA,ONE,solo,,3,,\n",
    ));

    assert_eq!(preview.warnings.len(), 1);
}

#[test]
fn test_preview_pooled_given_near_duplicate_team_names_returns_one_warning_with_both_keys() {
    let preview = run_on(
        &pooled_roster(),
        &csv(
            "Ada,One,team,Fake BNBA1,10,,\nBo,Two,team,Fake BNBA1,11,,\nCy,Three,team,Fake BNBA 1,12,,\nDi,Four,team,Fake BNBA 1,13,,\n",
        ),
    );

    assert_eq!(counts(&preview), (4, 2, 2, 0, 1));
    assert_eq!(
        preview.warnings,
        vec![conflict(
            4,
            ProblemCode::NearDuplicateTeamName,
            "possible duplicate team name: \"fake bnba1\" and \"fake bnba 1\""
        )]
    );
}

#[test]
fn test_preview_pooled_given_a_converted_one_rider_team_skips_its_near_duplicate_warning() {
    let preview = run_with(
        &pooled_roster(),
        &csv(
            "Ada,One,team,Fake BNBA1,10,,\nCy,Three,team,Fake BNBA 1,12,,\nDi,Four,team,Fake BNBA 1,13,,\n",
        ),
        convert_only(),
    );

    assert_eq!(counts(&preview), (3, 1, 2, 0, 0));
}

#[test]
fn test_preview_given_warnings_out_of_file_order_returns_them_sorted_by_row() {
    let preview = run(&csv(
        "Ada,One,solo,,1,,\nada,one,solo,,2,,\nBo,Two,team,Lonely,3,,\n",
    ));

    assert_eq!(problem_rows(&preview.warnings), vec![3, 4]);
    assert_eq!(
        problem_codes(&preview.warnings),
        vec![ProblemCode::DuplicateRiderName, ProblemCode::TeamUnderMin]
    );
}

#[test]
fn test_preview_given_conflicts_out_of_file_order_returns_them_sorted_by_row() {
    let roster = roster_of(PlateModel::TeamRelay, RideStatus::Running, Vec::new());

    let preview = run_on(&roster, &csv("Ada,One,team,Lonely,1,,\n,,solo,,7,,\n"));

    assert_eq!(problem_rows(&preview.conflicts), vec![2, 3]);
    assert_eq!(
        problem_codes(&preview.conflicts),
        vec![ProblemCode::TeamUnderMin, ProblemCode::MissingName]
    );
}

// =========================================== counts, fingerprint and purity

#[test]
fn test_preview_given_entries_counts_the_riders_teams_and_entries() {
    let preview = run(&csv(
        "Ada,One,solo,,1,,\nBo,Two,solo,,2,,\nCy,Three,team,Wolves,3,,\nDi,Four,team,Wolves,3,,\n",
    ));

    assert_eq!(counts(&preview), (4, 1, 3, 0, 0));
}

#[test]
fn test_preview_given_fixture_sets_the_source_path_to_the_file_argument() {
    let preview = fixture(
        "clean_pooled.csv",
        include_str!("../../fixtures/clean_pooled.csv"),
        &pooled_roster(),
    );

    assert_eq!(preview.source_path, PathBuf::from("clean_pooled.csv"));
}

#[test]
fn test_preview_given_a_roster_sets_the_fingerprint_from_it() {
    let mut roster = relay_roster();
    roster
        .create_solo_entry(rider("Ada", "Lovelace", None), "1")
        .expect("the plate is free");
    roster
        .create_solo_entry(rider("Alan", "Turing", None), "2")
        .expect("the plate is free");
    roster
        .create_team_entry(TeamFields {
            display_name: "Wolves",
            riders: vec![
                rider("Grace", "Hopper", None),
                rider("Ken", "Thompson", None),
            ],
            plate: Some("5"),
            logo_card: None,
        })
        .expect("the team is valid");

    let preview = run_on(&roster, &csv(""));

    assert_eq!(
        preview.fingerprint,
        PreviewFingerprint {
            audit_len: 3,
            entries: 3,
            riders: 4,
        }
    );
}

#[test]
fn test_preview_leaves_the_roster_and_its_audit_log_unchanged() {
    let mut roster = relay_roster();
    roster
        .create_solo_entry(rider("Ada", "Lovelace", None), "1")
        .expect("the plate is free");
    roster
        .create_team_entry(TeamFields {
            display_name: "Wolves",
            riders: vec![
                rider("Grace", "Hopper", None),
                rider("Ken", "Thompson", None),
            ],
            plate: Some("5"),
            logo_card: None,
        })
        .expect("the team is valid");
    let audit_before = roster.audit_log().to_vec();
    let entries_before: Vec<(String, String, String, usize)> = roster
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.key.clone(),
                entry.plate.clone(),
                entry.display_name.clone(),
                entry.riders.len(),
            )
        })
        .collect();
    let next_plate_before = roster.next_free_plate();

    run_on(&roster, &csv("Cy,Three,solo,,9,,\n"));

    let entries_after: Vec<(String, String, String, usize)> = roster
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.key.clone(),
                entry.plate.clone(),
                entry.display_name.clone(),
                entry.riders.len(),
            )
        })
        .collect();
    assert_eq!(entries_after, entries_before);
    assert_eq!(roster.audit_log(), audit_before);
    assert_eq!(roster.next_free_plate(), next_plate_before);
}

#[test]
fn test_preview_given_a_file_reads_and_previews_it() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("entries.csv");
    let payload = csv("Ada,Lovelace,solo,,7,,\n");
    std::fs::write(&path, &payload)?;

    let preview = preview(&path, &relay_roster(), options())?;

    assert_eq!((preview.riders, preview.entries.len()), (1, 1));
    assert_eq!(std::fs::read(&path)?, payload.into_bytes());
    Ok(())
}

#[test]
fn test_preview_given_a_missing_file_returns_io_error() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("absent.csv");

    let error = preview(&path, &relay_roster(), options()).expect_err("a missing file is refused");

    assert!(matches!(error, CsvIoError::Io(_)));
    Ok(())
}

// ================================================================ fixtures

#[test]
fn test_preview_clean_180_fixture_reports_the_pinned_counts() {
    let preview = fixture(
        "clean_180.csv",
        include_str!("../../fixtures/clean_180.csv"),
        &relay_roster(),
    );

    assert_eq!(counts(&preview), (180, 15, 135, 0, 16));
    assert!(
        preview
            .warnings
            .iter()
            .all(|warning| warning.code == ProblemCode::DuplicateRiderName)
    );
}

#[test]
fn test_preview_clean_pooled_fixture_reports_the_pinned_counts() {
    let preview = fixture(
        "clean_pooled.csv",
        include_str!("../../fixtures/clean_pooled.csv"),
        &pooled_roster(),
    );

    assert_eq!(counts(&preview), (9, 2, 6, 0, 0));
}

#[test]
fn test_preview_clean_pooled_fixture_gives_the_falcons_entry_its_lowest_plate() {
    let preview = fixture(
        "clean_pooled.csv",
        include_str!("../../fixtures/clean_pooled.csv"),
        &pooled_roster(),
    );

    assert_eq!(preview.entries[4].display_name, "Falcons");
    assert_eq!(preview.entries[4].plate, "10");
    assert_eq!(
        preview.entries[4]
            .riders
            .iter()
            .map(|rider| rider.plate.clone())
            .collect::<Vec<_>>(),
        vec![
            Some("10".to_owned()),
            Some("11".to_owned()),
            Some("12".to_owned())
        ]
    );
}

#[test]
fn test_preview_dup_plate_fixture_reports_one_conflict_at_row_three() {
    let preview = fixture(
        "dup_plate.csv",
        include_str!("../../fixtures/dup_plate.csv"),
        &relay_roster(),
    );

    assert_eq!(counts(&preview), (2, 0, 2, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            3,
            ProblemCode::DuplicatePlate,
            "duplicate plate 1"
        )]
    );
}

#[test]
fn test_preview_missing_name_fixture_reports_one_conflict_at_row_two() {
    let preview = fixture(
        "missing_name.csv",
        include_str!("../../fixtures/missing_name.csv"),
        &relay_roster(),
    );

    assert_eq!(counts(&preview), (0, 0, 0, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(2, ProblemCode::MissingName, "missing name")]
    );
}

#[test]
fn test_preview_team_over_max_fixture_reports_one_conflict_at_row_two() {
    let preview = fixture(
        "team_over_max.csv",
        include_str!("../../fixtures/team_over_max.csv"),
        &relay_roster(),
    );

    assert_eq!(counts(&preview), (5, 1, 1, 1, 0));
    assert_eq!(
        preview.conflicts,
        vec![conflict(
            2,
            ProblemCode::TeamOverMax,
            "team of 5 riders exceeds the maximum of 4 (team-over-max)"
        )]
    );
}

#[test]
fn test_preview_team_under_min_pooled_fixture_reports_one_warning_at_row_five() {
    let preview = fixture(
        "team_under_min_pooled.csv",
        include_str!("../../fixtures/team_under_min_pooled.csv"),
        &pooled_roster(),
    );

    assert_eq!(counts(&preview), (4, 2, 3, 0, 1));
    assert_eq!(
        preview.warnings,
        vec![conflict(5, ProblemCode::TeamUnderMin, UNDER_MIN_ONE)]
    );
}

#[test]
fn test_preview_gorba_fake_fixture_reports_the_pinned_counts() {
    let preview = fixture(
        "gorba_fake.csv",
        include_str!("../../fixtures/gorba_fake.csv"),
        &relay_roster(),
    );

    assert_eq!(counts(&preview), (10, 5, 8, 0, 5));
    assert_eq!(
        problem_codes(&preview.warnings),
        vec![
            ProblemCode::DuplicateRiderName,
            ProblemCode::TeamUnderMin,
            ProblemCode::TeamUnderMin,
            ProblemCode::NearDuplicateTeamName,
            ProblemCode::TeamUnderMin,
        ]
    );
}

#[test]
fn test_preview_gorba_epic_scrubbed_fixture_reports_the_pinned_counts() {
    let preview = fixture(
        "gorba_epic_scrubbed.csv",
        include_str!("../../fixtures/gorba_epic_scrubbed.csv"),
        &relay_roster(),
    );

    assert_eq!(counts(&preview), (101, 31, 71, 0, 12));
}

#[test]
fn test_preview_gorba_epic_scrubbed_fixture_pins_the_warning_rows_and_codes() {
    let preview = fixture(
        "gorba_epic_scrubbed.csv",
        include_str!("../../fixtures/gorba_epic_scrubbed.csv"),
        &relay_roster(),
    );
    let under_min: Vec<usize> = preview
        .warnings
        .iter()
        .filter(|warning| warning.code == ProblemCode::TeamUnderMin)
        .map(|warning| warning.row)
        .collect();
    let near_duplicate: Vec<usize> = preview
        .warnings
        .iter()
        .filter(|warning| warning.code == ProblemCode::NearDuplicateTeamName)
        .map(|warning| warning.row)
        .collect();

    assert_eq!(under_min, vec![17, 36, 41, 45, 59, 84, 85, 86]);
    assert_eq!(near_duplicate, vec![13, 20, 37, 85]);
}

#[test]
fn test_preview_gorba_epic_scrubbed_fixture_allocates_unique_numeric_plates() {
    let preview = fixture(
        "gorba_epic_scrubbed.csv",
        include_str!("../../fixtures/gorba_epic_scrubbed.csv"),
        &relay_roster(),
    );
    let mut unique: Vec<&str> = plates(&preview);
    unique.sort_unstable();
    unique.dedup();

    assert!(
        plates(&preview)
            .iter()
            .all(|plate| plate.chars().all(|c| c.is_ascii_digit()))
    );
    assert_eq!(unique.len(), 71);
}

// ============================================================ plate helper

#[rstest]
#[case("1", "2")]
#[case("9", "10")]
#[case("19", "20")]
#[case("99", "100")]
#[case("0099", "100")]
#[case("10", "11")]
#[case("", "1")]
fn test_preview_next_plate_given_digits_returns_the_successor(
    #[case] digits: &str,
    #[case] expected: &str,
) {
    assert_eq!(next_plate(digits), expected);
}

#[test]
fn test_preview_compare_plates_given_different_lengths_orders_by_value() {
    assert_eq!(compare_plates("9", "10"), Ordering::Less);
}

#[test]
fn test_preview_compare_plates_given_a_numeric_tie_orders_lexicographically() {
    assert_eq!(compare_plates("07", "7"), Ordering::Less);
    assert_eq!(compare_plates("7", "7"), Ordering::Equal);
}
