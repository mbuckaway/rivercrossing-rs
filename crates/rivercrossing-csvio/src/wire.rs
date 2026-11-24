// SPDX-License-Identifier: GPL-3.0-only

//! The CSV wire layer: read bytes into records and write records to bytes.
//!
//! The reader and the writer reproduce the Python `csv` module's `excel`
//! dialect: CRLF terminators on write, the formula guard on every cell, and no
//! automatic type conversion.

use crate::error::CsvIoError;

/// The `UTF-8` byte-order mark that a spreadsheet may put before the header.
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// The largest field, in bytes, that the reader accepts.
pub(crate) const MAX_FIELD_BYTES: usize = 131_072;

/// The refusal of an oversized field.
const FIELD_TOO_LARGE: &str = "malformed CSV data: field too large";

/// The characters that a spreadsheet reads as the start of a formula.
const FORMULA_LEADERS: [char; 6] = ['=', '+', '-', '@', '\t', '\r'];

/// Read every record of a CSV payload.
///
/// A leading `UTF-8` byte-order mark is removed first. A blank line carries no
/// record, so the reader drops it, which matches the import's skip of a
/// name-less row.
///
/// # Errors
///
/// Return [`CsvIoError::NotUtf8`] when the payload is not valid `UTF-8`, and
/// [`CsvIoError::Malformed`] when a field holds more than [`MAX_FIELD_BYTES`]
/// bytes or the reader reports a failure.
pub(crate) fn read_records(bytes: &[u8]) -> Result<Vec<Vec<String>>, CsvIoError> {
    let payload = bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes);
    std::str::from_utf8(payload).map_err(|_| CsvIoError::NotUtf8)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(payload);
    let mut records = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|error| malformed(&error))?;
        ensure_fields_fit(&record)?;
        records.push(record.iter().map(str::to_owned).collect());
    }
    Ok(records)
}

/// Write records as CRLF-terminated CSV bytes.
///
/// Every cell passes the formula guard, including a header cell, so a
/// spreadsheet never runs a cell that starts with a formula leader.
///
/// # Errors
///
/// Return [`CsvIoError::Malformed`] when the writer refuses a record.
pub(crate) fn write_records(records: &[Vec<String>]) -> Result<Vec<u8>, CsvIoError> {
    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(Vec::new());
    for record in records {
        let guarded: Vec<String> = record.iter().map(|cell| neutralise_formula(cell)).collect();
        writer
            .write_record(&guarded)
            .map_err(|error| malformed(&error))?;
    }
    writer.into_inner().map_err(|error| CsvIoError::Malformed {
        message: format!("malformed CSV data: {}", error.error()),
    })
}

/// Return *cell* with the formula guard applied.
///
/// A cell that starts with a formula leader gets a leading apostrophe, so a
/// spreadsheet reads it as text. A leading space is not neutralised, because
/// the guard prefixes and never strips.
#[must_use]
pub(crate) fn neutralise_formula(cell: &str) -> String {
    if cell.starts_with(FORMULA_LEADERS) {
        return format!("'{cell}");
    }
    cell.to_owned()
}

/// Return the malformed-payload error for a CSV reader or writer failure.
fn malformed(error: &csv::Error) -> CsvIoError {
    CsvIoError::Malformed {
        message: format!("malformed CSV data: {error}"),
    }
}

/// Return an error when any field of *record* is too large.
fn ensure_fields_fit(record: &csv::StringRecord) -> Result<(), CsvIoError> {
    if record.iter().any(|field| field.len() > MAX_FIELD_BYTES) {
        return Err(CsvIoError::Malformed {
            message: FIELD_TOO_LARGE.to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
