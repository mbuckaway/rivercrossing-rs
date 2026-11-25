// SPDX-License-Identifier: GPL-3.0-only

//! The rider-issues dialog's rows, action gating and the three one-click
//! fixes.
//!
//! The rules and the copy follow the Python oracle's
//! `ui/presenters/rider_issues.py`. The report itself is the advisory scan in
//! [`rivercrossing_rider_issues::checks`]; this module renders the kinds that
//! carry a fix and gates each fix on the roster's own lock matrix. Every plate
//! write goes through [`Roster::change_plate`], so the model-correct primitive
//! is chosen from the row's entry and rider.

use rivercrossing_ride::EntryId;
use rivercrossing_rider_issues::checks;
use rivercrossing_rider_issues::issue::{IssueKind as CheckedKind, RiderIssue};
use rivercrossing_roster::enums::PlateModel;
use rivercrossing_roster::lock::can_edit_structure;
use rivercrossing_roster::roster::{PlateChange, Roster};
use rivercrossing_roster::types::Rider;

#[cfg(test)]
mod tests;

/// The audit reason that a conversion records; the roster ignores it.
const CONVERT_REASON: &str = "rider issues: convert to solo";

/// The refusal for a row that no longer denotes a reported issue.
const STALE_ROW: &str = "the selected issue is no longer in the roster";

/// The refusal for a conversion of a row that is not a team-of-one.
const CONVERT_NOT_TEAM: &str = "only a team-of-one can be converted to solo";

/// The refusal for a conversion of a team that holds no rider.
const CONVERT_EMPTY: &str = "an empty team has no rider to convert to solo";

/// The refusal for a conversion on a ride that does not pool plates.
const CONVERT_NOT_POOLED: &str = "convert to solo requires a rider-pooled ride";

/// The refusal for a conversion once the ride has left DRAFT.
const CONVERT_LOCKED: &str = "a team-of-one can only be converted while the ride is draft";

/// The refusal for the assign fix on another kind of row.
const ASSIGN_NOT_MISSING: &str = "select a missing-number issue to assign a plate";

/// The refusal for the assign fix once the ride has left DRAFT.
const ASSIGN_LOCKED: &str = "plates can only be assigned while the ride is draft";

/// The refusal for the renumber fix on another kind of row.
const RENUMBER_NOT_DUPLICATE: &str = "select a duplicate-number issue to renumber";

/// The refusal for the renumber fix once the ride has left DRAFT.
const RENUMBER_LOCKED: &str = "plates can only be changed while the ride is draft";

/// The kinds of defect that the dialog reports.
///
/// The checker's two team-name warnings carry no fix, so the dialog does not
/// list them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IssueKind {
    /// A team below the minimum team size.
    TeamOfOne,
    /// A rider whose full name is empty.
    MissingName,
    /// A pooled rider whose plate is absent or blank.
    MissingNumber,
    /// A rider whose name another rider already claims.
    DuplicateName,
    /// A plate that another rider or entry already claims.
    DuplicateNumber,
}

impl IssueKind {
    /// Return the oracle's spelling of this kind.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::TeamOfOne => "team-of-one",
            Self::MissingName => "missing-name",
            Self::MissingNumber => "missing-number",
            Self::DuplicateName => "duplicate-name",
            Self::DuplicateNumber => "duplicate-number",
        }
    }
}

/// One issue-list row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IssueRow {
    /// The kind of defect.
    pub(crate) kind: IssueKind,
    /// The offending entry's plate.
    pub(crate) plate: String,
    /// The Name cell: the entry's name, else the rider's.
    pub(crate) name: String,
    /// The operator-facing text.
    pub(crate) detail: String,
}

/// The three fixes' enablement for one selected row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IssueActions {
    /// The editor a row opens: the teams editor or the rider editor.
    pub(crate) open_editor: Option<&'static str>,
    /// Whether Convert to Solo is enabled.
    pub(crate) convert_solo: bool,
    /// Whether Assign Plate is enabled.
    pub(crate) assign_plate: bool,
    /// Whether Renumber is enabled.
    pub(crate) renumber: bool,
}

/// Return every supported defect, in the checker's own order.
///
/// The two team-name duplicate warnings are omitted, because they carry no
/// fix.
#[must_use]
pub(crate) fn rows(roster: &Roster) -> Vec<IssueRow> {
    checks::rider_issues(roster)
        .into_iter()
        .filter_map(|issue| {
            let kind = kind_of(issue.kind)?;
            Some(IssueRow {
                kind,
                plate: issue.entry.plate.clone(),
                name: row_name(&issue),
                detail: issue.message,
            })
        })
        .collect()
}

/// Return the fixes that *row* currently offers on *roster*.
///
/// The conversion needs a pooled ride, a rider to extract and DRAFT; the two
/// plate fixes need their own kind and DRAFT. The editor choice follows the
/// kind: only a team-of-one opens the teams editor.
#[must_use]
pub(crate) fn actions(roster: &Roster, row: &IssueRow) -> IssueActions {
    let editable = can_edit_structure(roster.status());
    let convertible = row.kind == IssueKind::TeamOfOne
        && roster.plate_model() == PlateModel::RiderPooled
        && editable
        && find_issue(roster, row).is_some_and(|issue| !issue.entry.riders.is_empty());
    IssueActions {
        open_editor: Some(if row.kind == IssueKind::TeamOfOne {
            "team"
        } else {
            "rider"
        }),
        convert_solo: convertible,
        assign_plate: row.kind == IssueKind::MissingNumber && editable,
        renumber: row.kind == IssueKind::DuplicateNumber && editable,
    }
}

/// Convert the team-of-one that *row* names into a solo entry.
///
/// The team's lone rider keeps their own plate, and the emptied team
/// dissolves. Return the new entry's id.
///
/// # Errors
///
/// Return the not-a-team-of-one refusal for another kind, the stale-row
/// refusal when no issue matches, the empty-team refusal, the
/// not-rider-pooled refusal, the DRAFT-only refusal, and the roster's own
/// refusal.
pub(crate) fn convert_to_solo(roster: &mut Roster, row: &IssueRow) -> Result<EntryId, String> {
    if row.kind != IssueKind::TeamOfOne {
        return Err(CONVERT_NOT_TEAM.to_owned());
    }
    let plate = convertible_plate(roster, row)?;
    roster
        .extract_rider_to_solo(&plate, CONVERT_REASON)
        .map_err(|error| error.to_string())
}

/// Assign *plate* to the missing-number rider that *row* names.
///
/// # Errors
///
/// Return the not-a-missing-number refusal for another kind, the DRAFT-only
/// refusal, the stale-row refusal, and the roster's own refusal (a taken or
/// blank plate).
pub(crate) fn assign_plate(roster: &mut Roster, row: &IssueRow, plate: &str) -> Result<(), String> {
    if row.kind != IssueKind::MissingNumber {
        return Err(ASSIGN_NOT_MISSING.to_owned());
    }
    if !can_edit_structure(roster.status()) {
        return Err(ASSIGN_LOCKED.to_owned());
    }
    change_row_plate(roster, row, plate)
}

/// Renumber the later duplicate claimant that *row* names.
///
/// # Errors
///
/// Return the not-a-duplicate-number refusal for another kind, the DRAFT-only
/// refusal, the stale-row refusal, and the roster's own refusal (a taken or
/// blank plate).
pub(crate) fn renumber(roster: &mut Roster, row: &IssueRow, plate: &str) -> Result<(), String> {
    if row.kind != IssueKind::DuplicateNumber {
        return Err(RENUMBER_NOT_DUPLICATE.to_owned());
    }
    if !can_edit_structure(roster.status()) {
        return Err(RENUMBER_LOCKED.to_owned());
    }
    change_row_plate(roster, row, plate)
}

// --------------------------------------------------------------- helpers

/// Return the plate of the team-of-one's lone rider, after the gate checks.
fn convertible_plate(roster: &Roster, row: &IssueRow) -> Result<String, String> {
    let Some(issue) = find_issue(roster, row) else {
        return Err(STALE_ROW.to_owned());
    };
    if issue.entry.riders.is_empty() {
        return Err(CONVERT_EMPTY.to_owned());
    }
    if roster.plate_model() != PlateModel::RiderPooled {
        return Err(CONVERT_NOT_POOLED.to_owned());
    }
    if !can_edit_structure(roster.status()) {
        return Err(CONVERT_LOCKED.to_owned());
    }
    Ok(issue
        .entry
        .riders
        .first()
        .and_then(|rider| rider.plate.clone())
        .unwrap_or_default())
}

/// Write *plate* onto the entry and rider that *row* names.
fn change_row_plate(roster: &mut Roster, row: &IssueRow, plate: &str) -> Result<(), String> {
    let Some(issue) = find_issue(roster, row) else {
        return Err(STALE_ROW.to_owned());
    };
    let entry = issue.entry.id();
    let rider_plate = issue.rider.and_then(|rider| rider.plate.clone());
    roster
        .change_plate(PlateChange {
            entry: &entry,
            rider_plate: rider_plate.as_deref(),
            plate,
        })
        .map_err(|error| error.to_string())
}

/// Return the reported issue that *row* denotes.
///
/// A row is a value, so the issue is found by its own fields: the kind, the
/// entry's plate, the Name cell and the detail text.
fn find_issue<'a>(roster: &'a Roster, row: &IssueRow) -> Option<RiderIssue<'a>> {
    checks::rider_issues(roster).into_iter().find(|issue| {
        kind_of(issue.kind) == Some(row.kind)
            && issue.entry.plate == row.plate
            && row_name(issue) == row.name
            && issue.message == row.detail
    })
}

/// Return the reported kind, or `None` for a kind the dialog omits.
fn kind_of(kind: CheckedKind) -> Option<IssueKind> {
    match kind {
        CheckedKind::TeamOfOne => Some(IssueKind::TeamOfOne),
        CheckedKind::MissingName => Some(IssueKind::MissingName),
        CheckedKind::MissingNumber => Some(IssueKind::MissingNumber),
        CheckedKind::DuplicateName => Some(IssueKind::DuplicateName),
        CheckedKind::DuplicateNumber => Some(IssueKind::DuplicateNumber),
        CheckedKind::DuplicateTeamName | CheckedKind::NearDuplicateTeamName => None,
    }
}

/// Return one issue row's Name cell: the entry's name, else the rider's.
fn row_name(issue: &RiderIssue<'_>) -> String {
    if issue.entry.display_name.is_empty() {
        return issue.rider.map_or_else(String::new, Rider::full_name);
    }
    issue.entry.display_name.clone()
}
