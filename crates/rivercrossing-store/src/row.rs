// SPDX-License-Identifier: GPL-3.0-only

//! The row types that the store returns.

use rivercrossing_ride::RideStatus;
use time::Date;
use time::format_description::well_known::Iso8601;

use crate::error::StoreError;

#[cfg(test)]
mod tests;

/// One ride's library summary: the library's Ride, Date, Status and Entries
/// columns.
#[derive(Debug, Clone, PartialEq)]
pub struct RideRow {
    /// The ride's id.
    pub id: i64,
    /// The ride's name.
    pub name: String,
    /// The event date, a calendar day with no clock part.
    pub event_date: Date,
    /// The ride's lifecycle state.
    pub status: RideStatus,
    /// The number of live entries; retired rows are not counted.
    pub entries: u32,
}

/// Read the stored `ride.event_date` text.
///
/// # Errors
///
/// Return [`StoreError::InvalidRow`] when the text is not an ISO-8601
/// calendar date.
pub(crate) fn event_date_from_str(stored: &str) -> Result<Date, StoreError> {
    Date::parse(stored, &Iso8601::DATE).map_err(|_| StoreError::InvalidRow {
        table: "ride",
        column: "event_date",
        stored: stored.to_owned(),
    })
}

/// Read the stored `ride.status` text.
///
/// # Errors
///
/// Return [`StoreError::InvalidRow`] when the text is not a known status
/// spelling.
pub(crate) fn ride_status_from_str(stored: &str) -> Result<RideStatus, StoreError> {
    stored
        .parse::<RideStatus>()
        .map_err(|_| StoreError::InvalidRow {
            table: "ride",
            column: "status",
            stored: stored.to_owned(),
        })
}

/// Convert a stored live-entry count to a row's `entries` field.
///
/// # Errors
///
/// Return [`StoreError::OutOfRange`] when the count does not fit `u32`.
pub(crate) fn live_entries_from_i64(value: i64) -> Result<u32, StoreError> {
    u32::try_from(value).map_err(|_| StoreError::OutOfRange {
        field: "entry_count",
        value,
        max: u32::MAX,
    })
}
