// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the header-row resolution.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{HeaderMap, resolve};

/// Build header cells from string slices.
fn cells(headers: &[&str]) -> Vec<String> {
    headers.iter().copied().map(str::to_owned).collect()
}

#[test]
fn test_header_resolve_canonical_export_header_maps_every_field_in_order() {
    let header = cells(&[
        "FIRSTNAME",
        "LASTNAME",
        "TYPE",
        "TEAMNAME",
        "PLATE",
        "NOTES",
        "SEX",
    ]);

    let resolved = resolve(&header).expect("the canonical export header resolves");

    assert_eq!(
        resolved,
        HeaderMap {
            team_name: Some(3),
            kind: Some(2),
            first_name: Some(0),
            last_name: Some(1),
            sex: Some(6),
            plate: Some(4),
            notes: Some(5),
        }
    );
}

#[test]
fn test_header_resolve_gorba_header_maps_names_type_and_team() {
    let header = cells(&[
        "Date",
        "First Name",
        "Last Name",
        "Solo or Team",
        "Team Name",
        "Team Size",
        "Team Members",
        "Meal Preference",
        "T-Shirt Size",
        "Emergency Contact Name",
        "Emergency Contact Number (xxx-xxx-xxxx)",
        "Medical",
    ]);

    let resolved = resolve(&header).expect("the GORBA header resolves");

    assert_eq!(
        (
            resolved.first_name,
            resolved.last_name,
            resolved.kind,
            resolved.team_name,
            resolved.plate,
        ),
        (Some(1), Some(2), Some(3), Some(4), None)
    );
}

#[rstest]
#[case::plate("Plate")]
#[case::plate_number("Plate Number")]
#[case::plate_padded_run("plate  number")]
#[case::number("Number")]
#[case::bib("Bib")]
#[case::bib_padded("  BIB  ")]
fn test_header_resolve_plate_alias_claims_the_column(#[case] candidate: &str) {
    let header = cells(&["FIRSTNAME", candidate]);

    let resolved = resolve(&header).expect("a header with a name column resolves");

    assert_eq!(resolved.plate, Some(1));
}

#[rstest]
#[case::race_number("Race Number")]
#[case::joined_words("PlateNumber")]
#[case::reversed_words("Number Plate")]
#[case::plural("Plate Numbers")]
fn test_header_resolve_plate_lookalike_does_not_claim_the_column(#[case] candidate: &str) {
    let header = cells(&["FIRSTNAME", candidate]);

    let resolved = resolve(&header).expect("a header with a name column resolves");

    assert_eq!(resolved.plate, None);
}

#[test]
fn test_header_resolve_unisex_column_does_not_claim_sex() {
    let header = cells(&["FIRSTNAME", "Unisex"]);

    let resolved = resolve(&header).expect("a header with a name column resolves");

    assert_eq!(resolved.sex, None);
}

#[test]
fn test_header_resolve_rider_sex_column_claims_sex() {
    let header = cells(&["FIRSTNAME", "Rider Sex"]);

    let resolved = resolve(&header).expect("a header with a name column resolves");

    assert_eq!(resolved.sex, Some(1));
}

#[test]
fn test_header_resolve_noteworthy_column_does_not_claim_notes() {
    let header = cells(&["FIRSTNAME", "Noteworthy"]);

    let resolved = resolve(&header).expect("a header with a name column resolves");

    assert_eq!(resolved.notes, None);
}

#[test]
fn test_header_resolve_medical_notes_column_claims_notes() {
    let header = cells(&["FIRSTNAME", "Medical Notes"]);

    let resolved = resolve(&header).expect("a header with a name column resolves");

    assert_eq!(resolved.notes, Some(1));
}

#[test]
fn test_header_resolve_singular_note_column_claims_notes() {
    let header = cells(&["FIRSTNAME", "Rider Note"]);

    let resolved = resolve(&header).expect("a header with a name column resolves");

    assert_eq!(resolved.notes, Some(1));
}

#[test]
fn test_header_resolve_team_and_type_text_claims_the_column_as_team_name() {
    let header = cells(&["FIRSTNAME", "Teamname (solo or team)"]);

    let resolved = resolve(&header).expect("a header with a name column resolves");

    assert_eq!((resolved.team_name, resolved.kind), (Some(1), None));
}

#[test]
fn test_header_resolve_solo_only_text_does_not_claim_type() {
    let header = cells(&["FIRSTNAME", "Solo Entry"]);

    let resolved = resolve(&header).expect("a header with a name column resolves");

    assert_eq!(resolved.kind, None);
}

#[test]
fn test_header_resolve_repeated_field_keeps_the_first_column() {
    let header = cells(&["First Name", "First Name"]);

    let resolved = resolve(&header).expect("a repeated name column still resolves");

    assert_eq!(resolved.first_name, Some(0));
}

#[test]
fn test_header_resolve_repeated_fields_all_keep_their_first_column() {
    let header = cells(&[
        "FIRSTNAME",
        "LASTNAME",
        "TYPE",
        "TEAMNAME",
        "SEX",
        "PLATE",
        "NOTES",
        "FIRSTNAME",
        "LASTNAME",
        "TYPE",
        "TEAMNAME",
        "SEX",
        "PLATE",
        "NOTES",
    ]);

    let resolved = resolve(&header).expect("a header of repeated fields resolves");

    assert_eq!(
        resolved,
        HeaderMap {
            team_name: Some(3),
            kind: Some(2),
            first_name: Some(0),
            last_name: Some(1),
            sex: Some(4),
            plate: Some(5),
            notes: Some(6),
        }
    );
}

#[test]
fn test_header_resolve_first_name_only_header_is_valid() {
    let header = cells(&["First Name", "Team Name"]);

    let resolved = resolve(&header).expect("a first-name column alone is a valid header");

    assert_eq!((resolved.first_name, resolved.last_name), (Some(0), None));
}

#[test]
fn test_header_resolve_last_name_only_header_is_valid() {
    let header = cells(&["Last Name"]);

    let resolved = resolve(&header).expect("a last-name column alone is a valid header");

    assert_eq!((resolved.first_name, resolved.last_name), (None, Some(0)));
}

#[test]
fn test_header_resolve_no_name_column_returns_the_row_one_error() {
    let header = cells(&["Team Name", "Plate"]);

    let error = resolve(&header).expect_err("a header with no name column is refused");

    assert_eq!(
        error,
        "missing or malformed header: no first or last name column".to_owned()
    );
}

#[test]
fn test_header_resolve_empty_header_returns_the_row_one_error() {
    let error = resolve(&[]).expect_err("an empty header is refused");

    assert_eq!(
        error,
        "missing or malformed header: no first or last name column".to_owned()
    );
}
