// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the CSV import data model.

use std::path::PathBuf;

use pretty_assertions::assert_eq;
use rivercrossing_roster::enums::EntryType;

use super::{
    ImportOptions, ImportPreview, ImportProblem, ImportReport, ParsedEntry, ParsedRider,
    PreviewFingerprint, ProblemCode,
};

fn fixture_rider(first_name: &str, last_name: &str, plate: Option<&str>) -> ParsedRider {
    ParsedRider {
        first_name: first_name.to_owned(),
        last_name: last_name.to_owned(),
        plate: plate.map(str::to_owned),
        sex: None,
    }
}

#[test]
fn test_model_import_options_default_is_all_false() {
    assert_eq!(
        ImportOptions::default(),
        ImportOptions {
            map_unknown_sex_to_male: false,
            convert_teams_of_one_to_solo: false,
        }
    );
}

#[test]
fn test_model_import_options_keeps_each_opt_in() {
    let options = ImportOptions {
        map_unknown_sex_to_male: true,
        convert_teams_of_one_to_solo: false,
    };

    assert!(options.map_unknown_sex_to_male);
    assert!(!options.convert_teams_of_one_to_solo);
}

#[test]
fn test_model_parsed_rider_keeps_name_and_optional_cells() {
    let rider = ParsedRider {
        first_name: "Ada".to_owned(),
        last_name: "Lovelace".to_owned(),
        plate: Some("7".to_owned()),
        sex: None,
    };

    assert_eq!(rider.first_name, "Ada");
    assert_eq!(rider.last_name, "Lovelace");
    assert_eq!(rider.plate.as_deref(), Some("7"));
    assert_eq!(rider.sex, None);
}

#[test]
fn test_model_parsed_entry_keeps_team_fields() {
    let entry = ParsedEntry {
        plate: "12".to_owned(),
        display_name: "Moss Ridge Riders".to_owned(),
        kind: EntryType::Team,
        riders: vec![
            fixture_rider("Ada", "Lovelace", Some("12")),
            fixture_rider("Alan", "Turing", None),
        ],
        notes: "first; second".to_owned(),
    };

    assert_eq!(entry.plate, "12");
    assert_eq!(entry.display_name, "Moss Ridge Riders");
    assert_eq!(entry.kind, EntryType::Team);
    assert_eq!(entry.riders.len(), 2);
    assert_eq!(entry.notes, "first; second");
}

#[test]
fn test_model_import_preview_keeps_the_rows_problems_and_fingerprint() {
    let entry = ParsedEntry {
        plate: "12".to_owned(),
        display_name: "Moss Ridge Riders".to_owned(),
        kind: EntryType::Team,
        riders: vec![fixture_rider("Ada", "Lovelace", Some("12"))],
        notes: String::new(),
    };
    let conflict = ImportProblem {
        row: 3,
        code: ProblemCode::DuplicatePlate,
        message: "duplicate plate '12'".to_owned(),
    };
    let warning = ImportProblem {
        row: 5,
        code: ProblemCode::TeamUnderMin,
        message: "team of one".to_owned(),
    };
    let preview = ImportPreview {
        source_path: PathBuf::from("entries.csv"),
        riders: 2,
        teams: 1,
        entries: vec![entry.clone()],
        conflicts: vec![conflict.clone()],
        warnings: vec![warning.clone()],
        fingerprint: PreviewFingerprint {
            audit_len: 4,
            entries: 6,
            riders: 9,
        },
    };

    assert_eq!(preview.source_path, PathBuf::from("entries.csv"));
    assert_eq!(preview.riders, 2);
    assert_eq!(preview.teams, 1);
    assert_eq!(preview.entries, vec![entry]);
    assert_eq!(preview.conflicts, vec![conflict]);
    assert_eq!(preview.warnings, vec![warning]);
    assert_eq!(
        preview.fingerprint,
        PreviewFingerprint {
            audit_len: 4,
            entries: 6,
            riders: 9,
        }
    );
}

#[test]
fn test_model_import_preview_accepts_empty_lists() {
    let preview = ImportPreview {
        source_path: PathBuf::new(),
        riders: 0,
        teams: 0,
        entries: Vec::new(),
        conflicts: Vec::new(),
        warnings: Vec::new(),
        fingerprint: PreviewFingerprint {
            audit_len: 0,
            entries: 0,
            riders: 0,
        },
    };

    assert_eq!(preview.entries, Vec::new());
    assert_eq!(preview.conflicts, Vec::new());
    assert_eq!(preview.warnings, Vec::new());
}

#[test]
fn test_model_import_report_keeps_the_counts_and_the_audit_events() {
    let report = ImportReport {
        inserted: 3,
        updated: 1,
        moved: 2,
        extracted: 1,
        joined: 4,
        audit_events: Vec::new(),
    };

    assert_eq!(report.inserted, 3);
    assert_eq!(report.updated, 1);
    assert_eq!(report.moved, 2);
    assert_eq!(report.extracted, 1);
    assert_eq!(report.joined, 4);
    assert_eq!(report.audit_events, Vec::new());
}
