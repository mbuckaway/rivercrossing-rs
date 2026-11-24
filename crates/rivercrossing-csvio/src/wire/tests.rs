// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the CSV wire reader and writer.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{MAX_FIELD_BYTES, neutralise_formula, read_records, write_records};
use crate::error::CsvIoError;

#[test]
fn test_read_records_quoted_comma_keeps_one_field() {
    let records = read_records(b"a,\"b,c\",d\r\n").expect("the payload parses");

    assert_eq!(
        records,
        vec![vec!["a".to_owned(), "b,c".to_owned(), "d".to_owned()]]
    );
}

#[test]
fn test_read_records_crlf_input_splits_records() {
    let records = read_records(b"a,b\r\nc,d\r\n").expect("the payload parses");

    assert_eq!(
        records,
        vec![
            vec!["a".to_owned(), "b".to_owned()],
            vec!["c".to_owned(), "d".to_owned()]
        ]
    );
}

#[test]
fn test_read_records_strips_one_leading_utf8_bom() {
    let records = read_records(b"\xEF\xBB\xBFa,b\n").expect("the payload parses");

    assert_eq!(records, vec![vec!["a".to_owned(), "b".to_owned()]]);
}

/// The Python `csv` module closes an unclosed quote at end of input, and so
/// does the Rust reader; the port keeps that lenient behavior.
#[test]
fn test_read_records_unclosed_quote_closes_at_end_of_input() {
    let records = read_records(b"a,\"b").expect("the reader closes the quote at end of input");

    assert_eq!(records, vec![vec!["a".to_owned(), "b".to_owned()]]);
}

#[test]
fn test_read_records_ragged_rows_are_accepted() {
    let records = read_records(b"a,b\nc\n").expect("the payload parses");

    assert_eq!(
        records,
        vec![vec!["a".to_owned(), "b".to_owned()], vec!["c".to_owned()]]
    );
}

#[test]
fn test_read_records_field_at_the_limit_is_accepted() {
    let field = "x".repeat(MAX_FIELD_BYTES);

    let records = read_records(field.as_bytes()).expect("a field at the limit is accepted");

    assert_eq!(records, vec![vec![field]]);
}

#[test]
fn test_read_records_field_over_the_limit_is_refused() {
    let field = "x".repeat(MAX_FIELD_BYTES + 1);

    let error = read_records(field.as_bytes()).expect_err("a field over the limit is refused");

    let CsvIoError::Malformed { message } = error else {
        panic!("expected a malformed CSV error");
    };
    assert_eq!(message, "malformed CSV data: field too large");
}

#[test]
fn test_read_records_invalid_utf8_returns_not_utf8() {
    let error = read_records(&[b'a', 0xFF, b'b']).expect_err("invalid utf-8 is refused");

    assert!(matches!(error, CsvIoError::NotUtf8));
}

/// A blank line carries no record, which matches the import's skip of a
/// name-less row.
#[test]
fn test_read_records_blank_line_is_dropped() {
    let records = read_records(b"a,b\n\nc,d\n").expect("the payload parses");

    assert_eq!(
        records,
        vec![
            vec!["a".to_owned(), "b".to_owned()],
            vec!["c".to_owned(), "d".to_owned()]
        ]
    );
}

#[test]
fn test_read_records_empty_input_returns_no_records() {
    let records = read_records(b"").expect("an empty payload parses");

    assert_eq!(records, Vec::<Vec<String>>::new());
}

#[rstest]
#[case::equals("=SUM(A1)", "'=SUM(A1)")]
#[case::plus("+1", "'+1")]
#[case::minus("-1", "'-1")]
#[case::at("@cmd", "'@cmd")]
#[case::tab("\tname", "'\tname")]
#[case::carriage_return("\rname", "'\rname")]
#[case::plain("plain", "plain")]
#[case::leading_space(" plain", " plain")]
fn test_wire_neutralise_formula_guards_each_leader(#[case] cell: &str, #[case] expected: &str) {
    assert_eq!(neutralise_formula(cell), expected);
}

#[test]
fn test_write_records_comma_cell_is_quoted() {
    let bytes = write_records(&[vec!["a,b".to_owned()]]).expect("the writer accepts the record");

    assert_eq!(bytes, b"\"a,b\"\r\n".to_vec());
}

#[test]
fn test_write_records_quote_cell_is_doubled() {
    let bytes = write_records(&[vec!["a\"b".to_owned()]]).expect("the writer accepts the record");

    assert_eq!(bytes, b"\"a\"\"b\"\r\n".to_vec());
}

#[test]
fn test_write_records_newline_cell_is_quoted() {
    let bytes = write_records(&[vec!["a\nb".to_owned()]]).expect("the writer accepts the record");

    assert_eq!(bytes, b"\"a\nb\"\r\n".to_vec());
}

#[test]
fn test_write_records_empty_last_field_keeps_the_separator() {
    let record = vec!["a".to_owned(), String::new()];

    let bytes = write_records(&[record]).expect("the writer accepts the record");

    assert_eq!(bytes, b"a,\r\n".to_vec());
}

#[test]
fn test_write_records_joins_cells_with_a_comma() {
    let record = vec!["a".to_owned(), "b".to_owned()];

    let bytes = write_records(&[record]).expect("the writer accepts the record");

    assert_eq!(bytes, b"a,b\r\n".to_vec());
}

#[test]
fn test_write_records_terminates_every_record_with_crlf() {
    let records = vec![vec!["a".to_owned()], vec!["b".to_owned()]];

    let bytes = write_records(&records).expect("the writer accepts the records");

    assert_eq!(bytes, b"a\r\nb\r\n".to_vec());
}

#[test]
fn test_write_records_header_formula_cell_is_prefixed() {
    let header = vec!["FIRSTNAME".to_owned(), "=cmd".to_owned()];

    let bytes = write_records(&[header]).expect("the writer accepts the header");

    assert_eq!(bytes, b"FIRSTNAME,'=cmd\r\n".to_vec());
}

#[test]
fn test_write_records_leading_space_cell_is_untouched() {
    let record = vec![" a".to_owned(), " b".to_owned()];

    let bytes = write_records(&[record]).expect("the writer accepts the record");

    assert_eq!(bytes, b" a, b\r\n".to_vec());
}

#[test]
fn test_write_records_empty_input_returns_no_bytes() {
    let bytes = write_records(&[]).expect("the writer accepts no records");

    assert_eq!(bytes, Vec::<u8>::new());
}
