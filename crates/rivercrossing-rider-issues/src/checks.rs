// SPDX-License-Identifier: GPL-3.0-only

//! The roster checks that the report runs.
//!
//! Each check walks the roster on its own, so one rider can carry several
//! defects. The report runs the checks in a fixed order: cheapest structural
//! problems first, plate-namespace collisions last.

use std::collections::HashSet;

use rivercrossing_roster::enums::{EntryType, PlateModel};
use rivercrossing_roster::names::{rider_name_key, team_name_key};
use rivercrossing_roster::roster::{MIN_TEAM_SIZE, Roster};
use rivercrossing_roster::types::{Entry, Rider};

use crate::issue::{IssueKind, RiderIssue};

/// Return every defect that *roster* carries, in the report's fixed order.
///
/// The report scans only the live entries, in creation order, and each
/// entry's riders in list order. It is advisory: the start gate stays in the
/// roster crate.
#[must_use]
pub fn rider_issues(roster: &Roster) -> Vec<RiderIssue<'_>> {
    let mut issues = team_of_one_issues(roster);
    issues.extend(missing_name_issues(roster));
    issues.extend(missing_number_issues(roster));
    issues.extend(duplicate_name_issues(roster));
    issues.extend(team_name_issues(roster));
    issues.extend(duplicate_number_issues(roster));
    issues
}

/// Return one team-of-one issue per TEAM entry below the minimum size.
fn team_of_one_issues(roster: &Roster) -> Vec<RiderIssue<'_>> {
    roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Team)
        .filter(|entry| entry.team_size() < MIN_TEAM_SIZE as usize)
        .map(|entry| RiderIssue {
            entry,
            rider: None,
            kind: IssueKind::TeamOfOne,
            message: format!(
                "team size must be at least {MIN_TEAM_SIZE}, got {}",
                entry.team_size()
            ),
        })
        .collect()
}

/// Return one missing-name issue per rider whose full name is empty.
fn missing_name_issues(roster: &Roster) -> Vec<RiderIssue<'_>> {
    let mut issues = Vec::new();
    for entry in roster.entries() {
        for rider in &entry.riders {
            if rider.full_name().is_empty() {
                issues.push(RiderIssue {
                    entry,
                    rider: Some(rider),
                    kind: IssueKind::MissingName,
                    message: "missing name".to_owned(),
                });
            }
        }
    }
    issues
}

/// Return one missing-number issue per pooled rider whose plate is blank.
///
/// A relay rider is legitimately plateless, because the plate belongs to the
/// entry, so this check runs only on a pooled ride.
fn missing_number_issues(roster: &Roster) -> Vec<RiderIssue<'_>> {
    if roster.plate_model() != PlateModel::RiderPooled {
        return Vec::new();
    }
    let mut issues = Vec::new();
    for entry in roster.entries() {
        for rider in &entry.riders {
            if present_plate(rider.plate.as_deref()).is_none() {
                issues.push(RiderIssue {
                    entry,
                    rider: Some(rider),
                    kind: IssueKind::MissingNumber,
                    message: "missing plate".to_owned(),
                });
            }
        }
    }
    issues
}

/// Return one duplicate-name issue per later rider sharing a name key.
///
/// A blank name is a missing-name defect, so it never participates here.
fn duplicate_name_issues(roster: &Roster) -> Vec<RiderIssue<'_>> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut issues = Vec::new();
    for entry in roster.entries() {
        for rider in &entry.riders {
            let full_name = rider.full_name();
            if full_name.is_empty() {
                continue;
            }
            let key = rider_name_key(&rider.first_name, &rider.last_name);
            if !seen.insert(key) {
                issues.push(RiderIssue {
                    entry,
                    rider: Some(rider),
                    kind: IssueKind::DuplicateName,
                    message: format!("duplicate rider name {full_name}"),
                });
            }
        }
    }
    issues
}

/// Return the duplicate and near-duplicate team-name issues, in that order.
///
/// The hard duplicates come first and near-duplicates second, so a hard
/// duplicate never also reports as a near duplicate.
fn team_name_issues(roster: &Roster) -> Vec<RiderIssue<'_>> {
    let mut issues = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut unique: Vec<(String, &Entry)> = Vec::new();
    for entry in roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Team)
    {
        let normalized = normalized_team_name(&entry.display_name);
        if seen.insert(normalized.clone()) {
            unique.push((normalized, entry));
        } else {
            let display_name = &entry.display_name;
            issues.push(RiderIssue {
                entry,
                rider: None,
                kind: IssueKind::DuplicateTeamName,
                message: format!("duplicate team name \"{display_name}\""),
            });
        }
    }
    issues.extend(near_duplicate_team_name_issues(&unique));
    issues
}

/// Return *name*'s trimmed, whitespace-collapsed, lowercased identity.
fn normalized_team_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// One fuzzy team-name group: its first normalized name and every later one.
struct TeamNameGroup<'a> {
    /// The fuzzy key that groups the names.
    key: String,
    /// The group's first normalized name.
    first: String,
    /// The group's later names, each with the entry that owns it.
    later: Vec<(String, &'a Entry)>,
}

/// Return one warning per later name that shares a fuzzy team key.
fn near_duplicate_team_name_issues<'entry>(
    unique: &[(String, &'entry Entry)],
) -> Vec<RiderIssue<'entry>> {
    let mut groups: Vec<TeamNameGroup<'entry>> = Vec::new();
    for (name, entry) in unique {
        let key = team_name_key(name);
        match groups.iter_mut().find(|group| group.key == key) {
            Some(group) => group.later.push((name.clone(), *entry)),
            None => groups.push(TeamNameGroup {
                key,
                first: name.clone(),
                later: Vec::new(),
            }),
        }
    }
    let mut issues = Vec::new();
    for group in &groups {
        let first = &group.first;
        for (name, entry) in &group.later {
            issues.push(RiderIssue {
                entry,
                rider: None,
                kind: IssueKind::NearDuplicateTeamName,
                message: format!(
                    "⚠ warning: possible duplicate team name: \"{first}\" and \"{name}\""
                ),
            });
        }
    }
    issues
}

/// Return one plate claim per entry, in entry then rider order.
fn plate_claims(roster: &Roster) -> Vec<(&Entry, Option<&Rider>, &str)> {
    match roster.plate_model() {
        PlateModel::RiderPooled => rider_plate_claims(roster),
        PlateModel::TeamRelay => entry_plate_claims(roster),
    }
}

/// Return one claim per pooled rider that carries a plate.
fn rider_plate_claims(roster: &Roster) -> Vec<(&Entry, Option<&Rider>, &str)> {
    let mut claims = Vec::new();
    for entry in roster.entries() {
        for rider in &entry.riders {
            if let Some(plate) = present_plate(rider.plate.as_deref()) {
                claims.push((entry, Some(rider), plate));
            }
        }
    }
    claims
}

/// Return one claim per relay entry that carries a plate.
fn entry_plate_claims(roster: &Roster) -> Vec<(&Entry, Option<&Rider>, &str)> {
    roster
        .entries()
        .iter()
        .filter_map(|entry| {
            present_plate(Some(entry.plate.as_str())).map(|plate| (entry, None, plate))
        })
        .collect()
}

/// Return *plate* when it is present and not blank.
fn present_plate(plate: Option<&str>) -> Option<&str> {
    plate.filter(|plate| !plate.trim().is_empty())
}

/// Return one duplicate-number issue per later claim of one raw plate.
fn duplicate_number_issues(roster: &Roster) -> Vec<RiderIssue<'_>> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut issues = Vec::new();
    for (entry, rider, plate) in plate_claims(roster) {
        if !seen.insert(plate) {
            issues.push(RiderIssue {
                entry,
                rider,
                kind: IssueKind::DuplicateNumber,
                message: format!("duplicate plate {plate}"),
            });
        }
    }
    issues
}

#[cfg(test)]
mod tests;
