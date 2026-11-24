// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the cell-level normalization rules.

use pretty_assertions::assert_eq;
use rivercrossing_roster::enums::EntryType;
use rstest::rstest;

use super::{
    SexCell, canonical_person_name, classify_sex, collapse_whitespace, derive_kind,
    display_team_name, join_notes, normalize_team_name,
};

#[test]
fn test_normalize_collapse_whitespace_trims_and_collapses_runs() {
    assert_eq!(collapse_whitespace("  Full   Send  "), "Full Send");
}

#[test]
fn test_normalize_collapse_whitespace_blank_cell_returns_empty() {
    assert_eq!(collapse_whitespace(" \t\n "), "");
}

#[test]
fn test_normalize_team_name_folds_case_and_space_runs() {
    assert_eq!(normalize_team_name("  Full   Send "), "full send");
}

#[test]
fn test_normalize_team_name_bnba1_key_lacks_the_space() {
    assert_eq!(normalize_team_name("BNBA1"), "bnba1");
}

#[test]
fn test_normalize_team_name_bnba_1_key_keeps_the_space() {
    assert_eq!(normalize_team_name("BNBA 1"), "bnba 1");
}

#[test]
fn test_normalize_display_team_name_collapses_and_keeps_case() {
    assert_eq!(display_team_name("  Full   Send "), "Full Send");
}

#[rstest]
#[case::upper("JOHN", "John")]
#[case::lower("john", "John")]
#[case::padded("  john  ", "John")]
#[case::mixed_case("McDonald", "McDonald")]
#[case::lower_words(" van der berg ", "Van der berg")]
#[case::mixed_words("van der Berg", "van der Berg")]
fn test_normalize_canonical_person_name_applies_the_roster_casing(
    #[case] raw: &str,
    #[case] expected: &str,
) {
    assert_eq!(canonical_person_name(raw), expected);
}

#[rstest]
#[case::male_word("male", false, SexCell::Known(Some("M".to_owned())))]
#[case::male_letter("m", false, SexCell::Known(Some("M".to_owned())))]
#[case::male_upper("MALE", false, SexCell::Known(Some("M".to_owned())))]
#[case::female_word("female", false, SexCell::Known(Some("F".to_owned())))]
#[case::female_letter("F", false, SexCell::Known(Some("F".to_owned())))]
#[case::female_padded("  Female ", false, SexCell::Known(Some("F".to_owned())))]
#[case::blank("", false, SexCell::Known(None))]
#[case::blank_spaces("   ", false, SexCell::Known(None))]
#[case::unknown("unknown", false, SexCell::Invalid("unknown".to_owned()))]
#[case::unknown_padded("  X  ", false, SexCell::Invalid("X".to_owned()))]
#[case::optin_blank("", true, SexCell::Known(Some("M".to_owned())))]
#[case::optin_unknown("unknown", true, SexCell::Known(Some("M".to_owned())))]
#[case::optin_letter("x", true, SexCell::Known(Some("M".to_owned())))]
fn test_normalize_classify_sex_applies_the_alias_and_opt_in(
    #[case] raw: &str,
    #[case] map_unknown_to_male: bool,
    #[case] expected: SexCell,
) {
    assert_eq!(classify_sex(raw, map_unknown_to_male), expected);
}

#[rstest]
#[case::blank_with_team("", "Full Send", Ok(EntryType::Team))]
#[case::blank_without_team("", "   ", Ok(EntryType::Solo))]
#[case::solo_with_team_name("solo", "Full Send", Ok(EntryType::Solo))]
#[case::solo_upper("SOLO", "", Ok(EntryType::Solo))]
#[case::team_with_name("team", "Full Send", Ok(EntryType::Team))]
#[case::team_padded(" Team ", "Full Send", Ok(EntryType::Team))]
#[case::team_without_name("team", "", Err("missing name".to_owned()))]
#[case::team_with_blank_name("team", "   ", Err("missing name".to_owned()))]
#[case::unknown("triple", "", Err("unknown entry type \"triple\"".to_owned()))]
#[case::unknown_keeps_spelling("  Triple  ", "", Err("unknown entry type \"Triple\"".to_owned()))]
fn test_normalize_derive_kind_applies_the_type_rules(
    #[case] type_cell: &str,
    #[case] team_name: &str,
    #[case] expected: Result<EntryType, String>,
) {
    assert_eq!(derive_kind(type_cell, team_name), expected);
}

#[test]
fn test_normalize_join_notes_empty_slice_returns_empty() {
    assert_eq!(join_notes(&[]), "");
}

#[test]
fn test_normalize_join_notes_single_value_has_no_separator() {
    assert_eq!(join_notes(&["solo"]), "solo");
}

#[test]
fn test_normalize_join_notes_joins_trimmed_values_in_order() {
    assert_eq!(join_notes(&["first", "second"]), "first; second");
}

#[test]
fn test_normalize_join_notes_skips_blank_values() {
    assert_eq!(
        join_notes(&[" first ", "", "  ", "second"]),
        "first; second"
    );
}

#[test]
fn test_normalize_join_notes_only_blank_values_returns_empty() {
    assert_eq!(join_notes(&["  ", ""]), "");
}
