// SPDX-License-Identifier: GPL-3.0-only

//! The name keys: the rider key, the canonical person name, and the team key
//! that the CSV preview and the rider-issue report share.
//!
//! Each key keeps the Python oracle's behaviour for the registration name
//! domain. The oracle folds with Python's `str.casefold`; the port folds with
//! Unicode lowercasing, which matches the oracle on ASCII and Latin-1 names.
//! The two differ only on the casefold table's extra expansions, such as the
//! German sharp s.

/// The fuzzy team key drops these characters before it tokenizes.
const TEAM_QUOTES: [char; 6] = ['\'', '"', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}'];

/// The tokens that the fuzzy team key treats as a conjunction.
const TEAM_CONJUNCTIONS: [&str; 2] = ["and", "&"];

/// Return a case-folded, whitespace-collapsed rider-name key.
///
/// CSV import and the rider-issue check compare names through this key, so
/// `"Mary Anne  Knibbe"` and `"mary   anne knibbe"` share one key.
#[must_use]
pub fn rider_name_key(first_name: &str, last_name: &str) -> String {
    format!("{first_name} {last_name}")
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Return *raw*'s canonical display case.
///
/// The function trims the field, then capitalizes a field written in one
/// uniform case: `"john"` and `"JOHN"` both become `"John"`. A field with any
/// case mixture is returned exactly as typed, so a surname the operator
/// spelled deliberately, such as `"McDonald"`, survives.
#[must_use]
pub fn canonical_person_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.to_uppercase() == trimmed || trimmed.to_lowercase() == trimmed {
        return capitalize(trimmed);
    }
    trimmed.to_owned()
}

/// Return *raw* with its first character uppercased and the rest lowercased.
fn capitalize(raw: &str) -> String {
    let mut chars = raw.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut capitalized = first.to_uppercase().collect::<String>();
    capitalized.push_str(&chars.as_str().to_lowercase());
    capitalized
}

/// Return *name*'s fuzzy key for near-duplicate team detection.
///
/// The key folds case, drops apostrophes and quotes, drops the standalone
/// `and` and `&` tokens, then keeps only the alphanumeric characters and
/// lowercases them. So `"BNBA 1"` and `"BNBA1"` key to `"bnba1"`, and
/// `"Win Win More Win"` and `"Win Win and more Win"` to `"winwinmorewin"`.
#[must_use]
pub fn team_name_key(name: &str) -> String {
    let folded = name.to_lowercase();
    let unquoted: String = folded
        .chars()
        .filter(|character| !TEAM_QUOTES.contains(character))
        .collect();
    unquoted
        .split_whitespace()
        .filter(|token| !TEAM_CONJUNCTIONS.contains(token))
        .flat_map(|token| token.chars())
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests;
