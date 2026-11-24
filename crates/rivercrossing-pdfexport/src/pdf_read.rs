// SPDX-License-Identifier: GPL-3.0-only

//! A tolerant structural scanner for the PDFs the writer emits.
//!
//! The tests cannot read the content streams (they are compressed), so this
//! scanner reads only the parts a reader can see without inflating: the page
//! dictionaries, the media boxes and the document information dictionary. It
//! is a scanner, not a parser: every check searches for the name it wants and
//! tolerates whichever whitespace the writer chose, so an engine upgrade that
//! changes spacing does not break an assertion about structure.

/// The name that marks a page dictionary.
const PAGE: &[u8] = b"Page";

/// The keyword that opens the classic trailer dictionary.
const TRAILER: &[u8] = b"trailer";

/// The name of the media-box entry.
const MEDIA_BOX: &[u8] = b"/MediaBox";

/// The number of points a comparison may differ by and still match.
const POINT_TOLERANCE: f32 = 0.01;

/// Return the number of page dictionaries in *bytes*.
pub(crate) fn page_count(bytes: &[u8]) -> usize {
    let mut count = 0;
    let mut rest = bytes;
    while let Some(offset) = find(rest, b"/Type") {
        let after = offset + b"/Type".len();
        if is_page_dict(rest, after) {
            count += 1;
        }
        rest = rest.split_at(after).1;
    }
    count
}

/// Return whether *bytes* carries a media box of *w* by *h* points.
pub(crate) fn media_box_contains(bytes: &[u8], w: f32, h: f32) -> bool {
    let Some(at) = find(bytes, MEDIA_BOX) else {
        return false;
    };
    let rest = bytes.split_at(at + MEDIA_BOX.len()).1;
    let open = skip_whitespace(rest, 0);
    if rest.get(open) != Some(&b'[') {
        return false;
    }
    let Some(numbers) = media_box_numbers(rest.split_at(open).1) else {
        return false;
    };
    matches_box(&numbers, w, h)
}

/// Return whether the trailer dictionary carries *key*.
pub(crate) fn has_trailer_key(bytes: &[u8], key: &str) -> bool {
    let Some(at) = trailer_at(bytes) else {
        return false;
    };
    has_name(bytes.split_at(at).1, key)
}

/// Return whether the document information dictionary carries *key*.
pub(crate) fn info_has(bytes: &[u8], key: &str) -> bool {
    info_object(bytes).is_some_and(|info| has_name(info, key))
}

/// Return whether *name* appears as a PDF name anywhere in *bytes*.
///
/// The absence assertions scan the whole document, which the exporter allows:
/// no dictionary other than the information dictionary repeats those keys.
pub(crate) fn has_name(bytes: &[u8], name: &str) -> bool {
    let needle = name.as_bytes();
    let mut rest = bytes;
    while let Some(offset) = find(rest, needle) {
        let end = offset + needle.len();
        if rest.get(end).is_none_or(|byte| is_name_end(*byte)) {
            return true;
        }
        rest = rest.split_at(end).1;
    }
    false
}

/// Return whether the name that follows `/Type` in *bytes* is `Page`.
fn is_page_dict(bytes: &[u8], after: usize) -> bool {
    let start = skip_whitespace(bytes, after);
    if bytes.get(start) != Some(&b'/') {
        return false;
    }
    let name_start = start + 1;
    let name_end = name_start + PAGE.len();
    bytes.get(name_start..name_end) == Some(PAGE)
        && bytes.get(name_end).is_none_or(|byte| is_name_end(*byte))
}

/// Return the numbers inside the media-box array that *rest* opens.
fn media_box_numbers(rest: &[u8]) -> Option<Vec<f32>> {
    let close = find(rest, b"]")?;
    let opened = rest.split_at(close).0;
    let text = std::str::from_utf8(opened.split_at(1).1).ok()?;
    Some(
        text.split(|ch: char| ch.is_whitespace() || ch == ',')
            .filter_map(|token| token.parse::<f32>().ok())
            .collect(),
    )
}

/// Return whether *numbers* holds the box `0 0 w h` with four edges.
fn matches_box(numbers: &[f32], w: f32, h: f32) -> bool {
    match numbers {
        [left, bottom, right, top] => [(*left, 0.0), (*bottom, 0.0), (*right, w), (*top, h)]
            .into_iter()
            .all(|(actual, expected)| approx(actual, expected)),
        _ => false,
    }
}

/// Return whether *actual* and *expected* agree within the point tolerance.
fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < POINT_TOLERANCE
}

/// Return the index of the last trailer keyword in *bytes*.
fn trailer_at(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(TRAILER.len())
        .rposition(|window| window == TRAILER)
}

/// Return the document information dictionary's bytes.
fn info_object(bytes: &[u8]) -> Option<&[u8]> {
    let trailer = bytes.split_at(trailer_at(bytes)?).1;
    let info_at = find(trailer, b"/Info")? + b"/Info".len();
    let number = leading_number(trailer.split_at(info_at).1)?;
    let header = format!("{number} 0 obj");
    let object = bytes.split_at(find(bytes, header.as_bytes())?).1;
    Some(object.split_at(find(object, b"endobj")?).0)
}

/// Return the decimal number *bytes* starts with, after any whitespace.
fn leading_number(bytes: &[u8]) -> Option<u64> {
    let digits = bytes.split_at(skip_whitespace(bytes, 0)).1;
    let mut value: u64 = 0;
    let mut seen = false;
    for byte in digits {
        if !byte.is_ascii_digit() {
            break;
        }
        value = value
            .saturating_mul(10)
            .saturating_add(u64::from(*byte) - u64::from(b'0'));
        seen = true;
    }
    seen.then_some(value)
}

/// Return whether *byte* ends a PDF name.
fn is_name_end(byte: u8) -> bool {
    !(byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b'#')
}

/// Return the index of the first byte of *start* that is not whitespace.
fn skip_whitespace(bytes: &[u8], start: usize) -> usize {
    let mut index = start;
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

/// Return the first index of *needle* in *haystack*.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests;
