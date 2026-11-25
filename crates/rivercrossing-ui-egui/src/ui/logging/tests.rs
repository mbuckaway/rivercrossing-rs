// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the per-launch NDJSON log: naming, gating, appending and pruning.

use std::ffi::OsStr;
use std::path::Path;

use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use time::format_description::well_known::Rfc3339;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, SignedDuration, Time};

use super::LaunchLog;

/// Return the launch instant every fixture uses: 1970-01-01T00:00:00Z.
fn epoch() -> Result<OffsetDateTime, time::error::ComponentRange> {
    OffsetDateTime::from_unix_timestamp(0)
}

/// Return `hour` o'clock UTC on 2026-06-01, the fixture launch day.
fn june_first(hour: u8) -> Result<OffsetDateTime, time::error::ComponentRange> {
    let date = Date::from_calendar_date(2026, Month::June, 1)?;
    Ok(PrimitiveDateTime::new(date, Time::from_hms(hour, 0, 0)?).assume_utc())
}

/// Write 25 launch logs dated January 2026, the oldest first.
fn write_stale_logs(dir: &Path) -> std::io::Result<()> {
    for day in 1..=25 {
        let name = format!("rivercrossing-202601{day:02}-000000.ndjson");
        std::fs::write(dir.join(name), b"{}")?;
    }
    Ok(())
}

/// Return the sorted launch log names in `dir`.
fn launch_log_names(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if name.starts_with("rivercrossing-") && name.ends_with(".ndjson") {
            names.push(name);
        }
    }
    names.sort_unstable();
    Ok(names)
}

/// Parse every line of an NDJSON file body into a JSON value.
fn parse_lines(text: &str) -> Result<Vec<Value>, serde_json::Error> {
    let mut values = Vec::new();
    for line in text.lines() {
        values.push(serde_json::from_str(line)?);
    }
    Ok(values)
}

/// Return the RFC 3339 `at` field of one record, when it parses.
fn line_at(value: &Value) -> Option<OffsetDateTime> {
    let text = value.get("at")?.as_str()?;
    OffsetDateTime::parse(text, &Rfc3339).ok()
}

#[test]
fn test_open_writes_one_ndjson_file_with_the_launch_name() -> Result<(), Box<dyn std::error::Error>>
{
    // Arrange
    let dir = tempfile::tempdir()?;

    // Act
    let (log, warnings) = LaunchLog::open(dir.path(), false, epoch()?);

    // Assert
    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(
        log.path().file_name(),
        Some(OsStr::new("rivercrossing-19700101-000000.ndjson"))
    );
    assert_eq!(log.path().parent(), Some(dir.path()));
    assert!(log.path().is_file());
    assert!(!log.is_verbose());
    Ok(())
}

#[test]
fn test_event_appends_one_json_object_per_line() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = tempfile::tempdir()?;
    let (mut log, _warnings) = LaunchLog::open(dir.path(), false, epoch()?);

    // Act
    log.event("startup", json!({"version": "0.1.0"}));
    log.event("quit", json!({}));

    // Assert
    let text = std::fs::read_to_string(log.path())?;
    let values = parse_lines(&text)?;
    assert_eq!(values.len(), 2);
    assert_eq!(
        values.first().and_then(|value| value.get("event")),
        Some(&json!("startup"))
    );
    assert_eq!(
        values.get(1).and_then(|value| value.get("event")),
        Some(&json!("quit"))
    );
    assert_eq!(values.first().and_then(line_at), Some(epoch()?));
    assert_eq!(
        values.get(1).and_then(line_at),
        epoch()?.checked_add(SignedDuration::milliseconds(1))
    );
    Ok(())
}

#[test]
fn test_detail_is_written_only_when_verbose() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = tempfile::tempdir()?;
    let (mut log, _warnings) = LaunchLog::open(dir.path(), false, epoch()?);

    // Act
    log.detail("menu", json!({"menu": "File"}));

    // Assert
    let text = std::fs::read_to_string(log.path())?;
    assert_eq!(text, "");
    Ok(())
}

#[test]
fn test_detail_is_written_when_verbose() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = tempfile::tempdir()?;
    let (mut log, _warnings) = LaunchLog::open(dir.path(), true, epoch()?);

    // Act
    log.detail("menu", json!({"menu": "File"}));

    // Assert
    let text = std::fs::read_to_string(log.path())?;
    let values = parse_lines(&text)?;
    assert!(log.is_verbose());
    assert_eq!(values.len(), 1);
    assert_eq!(
        values.first().and_then(|value| value.get("event")),
        Some(&json!("menu"))
    );
    assert_eq!(
        values.first().and_then(|value| value.get("fields")),
        Some(&json!({"menu": "File"}))
    );
    Ok(())
}

#[test]
fn test_open_prunes_to_the_newest_twenty() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = tempfile::tempdir()?;
    write_stale_logs(dir.path())?;
    // Two names that only half-match the launch pattern: the prune must keep both.
    std::fs::write(dir.path().join("notes.txt"), b"keep")?;
    std::fs::write(dir.path().join("rivercrossing-notes.txt"), b"keep")?;
    let now = june_first(12)?;

    // Act
    let (_log, warnings) = LaunchLog::open(dir.path(), false, now);

    // Assert
    let mut expected: Vec<String> = (7..=25)
        .map(|day| format!("rivercrossing-202601{day:02}-000000.ndjson"))
        .collect();
    expected.push("rivercrossing-20260601-120000.ndjson".to_owned());
    expected.sort_unstable();
    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(launch_log_names(dir.path())?, expected);
    assert!(dir.path().join("notes.txt").is_file());
    assert!(dir.path().join("rivercrossing-notes.txt").is_file());
    Ok(())
}

#[test]
fn test_open_on_an_unwritable_directory_returns_a_warning_and_no_file()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = tempfile::tempdir()?;
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, b"not a directory")?;

    // Act
    let (log, warnings) = LaunchLog::open(&blocked, false, epoch()?);

    // Assert
    assert_eq!(warnings.len(), 1);
    assert_eq!(log.path().parent(), Some(blocked.as_path()));
    assert!(!log.path().exists());
    Ok(())
}

#[test]
fn test_open_on_an_occupied_launch_path_returns_a_warning_and_no_file()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = tempfile::tempdir()?;
    let occupied = dir.path().join("rivercrossing-19700101-000000.ndjson");
    std::fs::create_dir(&occupied)?;

    // Act
    let (log, warnings) = LaunchLog::open(dir.path(), false, epoch()?);

    // Assert
    assert_eq!(warnings.len(), 1);
    assert!(!log.path().is_file());
    Ok(())
}

#[test]
fn test_open_reports_a_log_it_cannot_prune() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = tempfile::tempdir()?;
    write_stale_logs(dir.path())?;
    let oldest = dir.path().join("rivercrossing-20240101-000000.ndjson");
    std::fs::create_dir(&oldest)?;
    let now = june_first(0)?;

    // Act
    let (_log, warnings) = LaunchLog::open(dir.path(), false, now);

    // Assert
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings
            .first()
            .is_some_and(|warning| warning.contains("could not prune"))
    );
    assert!(oldest.is_dir());
    Ok(())
}

#[test]
fn test_event_without_a_file_is_a_no_op() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = tempfile::tempdir()?;
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, b"not a directory")?;
    let (mut log, _warnings) = LaunchLog::open(&blocked, true, epoch()?);

    // Act
    log.event("startup", json!({"version": "0.1.0"}));

    // Assert
    assert!(!log.path().exists());
    Ok(())
}

#[test]
fn test_every_line_is_valid_json() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let dir = tempfile::tempdir()?;
    let (mut log, _warnings) = LaunchLog::open(dir.path(), true, epoch()?);

    // Act
    log.event("startup", json!({"note": "line one\nline two"}));
    log.detail("menu", json!({"menu": "File", "item": null}));

    // Assert
    let text = std::fs::read_to_string(log.path())?;
    let values = parse_lines(&text)?;
    assert_eq!(values.len(), 2);
    assert_eq!(text.lines().count(), 2);
    assert_eq!(
        values.first().and_then(|value| value.get("fields")),
        Some(&json!({"note": "line one\nline two"}))
    );
    assert_eq!(values.first().and_then(line_at), Some(epoch()?));
    Ok(())
}
