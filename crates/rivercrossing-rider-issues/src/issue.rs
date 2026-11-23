// SPDX-License-Identifier: GPL-3.0-only

//! One reported rider issue.

use rivercrossing_roster::types::{Entry, Rider};

/// The kind of defect that the report names.
///
/// This order is not the report order: [`crate::checks::rider_issues`] emits
/// the kinds in its own fixed sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueKind {
    /// A team below the minimum team size.
    TeamOfOne,
    /// A rider whose full name is empty.
    MissingName,
    /// A rider on a pooled ride whose plate is absent or blank.
    MissingNumber,
    /// A rider whose name another rider already claims.
    DuplicateName,
    /// A team whose normalized name another team already claims.
    DuplicateTeamName,
    /// A team whose fuzzy name key another team's name shares.
    NearDuplicateTeamName,
    /// A plate that another rider or entry already claims.
    DuplicateNumber,
}

/// One defect that a roster currently carries.
///
/// `rider` is the offending rider when the defect is rider-scoped, and `None`
/// when it is team- or entry-scoped. The report borrows the roster, so a
/// caller computes it on demand and never stores it across a roster mutation.
#[derive(Debug)]
pub struct RiderIssue<'a> {
    /// The offending entry.
    pub entry: &'a Entry,
    /// The offending rider, when the defect is rider-scoped.
    pub rider: Option<&'a Rider>,
    /// The kind of defect.
    pub kind: IssueKind,
    /// The operator-facing text.
    pub message: String,
}

#[cfg(test)]
mod tests;
