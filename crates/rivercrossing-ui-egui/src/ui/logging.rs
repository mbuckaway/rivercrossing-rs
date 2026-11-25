// SPDX-License-Identifier: GPL-3.0-only

//! The per-launch NDJSON log: one file per app start, beside the settings.
//!
//! Lifecycle records (startup, ride open and close, quit) are always written;
//! the operator's trace records are written only when verbose logging is on.
//! An unwritable directory yields a warning and no file: the launch log never
//! stops the app.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;
use time::{OffsetDateTime, SignedDuration, format_description::well_known::Rfc3339};

#[cfg(test)]
mod tests;

/// The basename every launch log shares.
const LOG_BASENAME: &str = "rivercrossing";

/// The suffix of a launch log.
const LOG_SUFFIX: &str = ".ndjson";

/// How many launch logs survive a prune.
const KEEP_LAUNCH_LOGS: usize = 20;

/// One app launch's NDJSON log.
#[derive(Debug)]
pub(crate) struct LaunchLog {
    /// The open file, or `None` when the directory could not be written.
    file: Option<File>,
    /// This launch's log path, set even when no file was opened.
    path: PathBuf,
    /// Whether trace records are written.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the verbose gate is read back by the launch-log tests"
        )
    )]
    verbose: bool,
    /// The launch instant that names the file and starts the record stamps.
    now: OffsetDateTime,
    /// How many records this log has written.
    sequence: i64,
}

impl LaunchLog {
    /// Open this launch's log in `dir` and prune it to the newest 20 files.
    ///
    /// Never fails: a directory that cannot be written returns no file and a
    /// warning.
    pub(crate) fn open(dir: &Path, verbose: bool, now: OffsetDateTime) -> (Self, Vec<String>) {
        let path = dir.join(format!("{LOG_BASENAME}-{}{LOG_SUFFIX}", launch_stamp(now)));
        let mut warnings = Vec::new();
        let file = open_file(dir, &path, &mut warnings);
        // Prune after creating this launch's file, so the new file counts
        // toward the twenty that stay.
        warnings.extend(prune(dir));
        (
            Self {
                file,
                path,
                verbose,
                now,
                sequence: 0,
            },
            warnings,
        )
    }

    /// Append one lifecycle record, whatever the verbose setting.
    pub(crate) fn event(&mut self, name: &str, fields: Value) {
        self.record(name, fields);
    }

    /// Append one trace record when verbose logging is on.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the operator trace records arrive with the later screens"
        )
    )]
    pub(crate) fn detail(&mut self, name: &str, fields: Value) {
        if self.verbose {
            self.record(name, fields);
        }
    }

    /// Return this launch's log path, set even when no file was opened.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Return whether trace records are written.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the launch-log tests read the verbose gate back")
    )]
    pub(crate) fn is_verbose(&self) -> bool {
        self.verbose
    }

    /// Append one record as a single JSON object.
    fn record(&mut self, name: &str, fields: Value) {
        let mut record = serde_json::Map::new();
        record.insert("at".to_owned(), Value::String(self.next_stamp()));
        record.insert("event".to_owned(), Value::String(name.to_owned()));
        record.insert("fields".to_owned(), fields);
        self.write_line(&Value::Object(record).to_string());
    }

    /// Return the record's RFC 3339 stamp: the launch instant plus one
    /// millisecond per record, so the file is deterministic for a test.
    fn next_stamp(&mut self) -> String {
        let at = self
            .now
            .checked_add(SignedDuration::milliseconds(self.sequence))
            .unwrap_or(self.now);
        self.sequence = self.sequence.saturating_add(1);
        at.format(&Rfc3339)
            .unwrap_or_else(|_| at.unix_timestamp().to_string())
    }

    /// Append one line, or drop the handle once the sink refuses a write.
    fn write_line(&mut self, line: &str) {
        let failed = {
            let Some(file) = self.file.as_mut() else {
                return;
            };
            writeln!(file, "{line}").is_err()
        };
        if failed {
            // The sink broke (a full disk, say). Drop the handle so later
            // records do not retry into it; the log never stops the app.
            self.file = None;
        }
    }
}

/// Create `dir` and this launch's file, recording each failure as a warning.
fn open_file(dir: &Path, path: &Path, warnings: &mut Vec<String>) -> Option<File> {
    if let Err(error) = std::fs::create_dir_all(dir) {
        warnings.push(format!("could not create {}: {error}", dir.display()));
        return None;
    }
    match File::create(path) {
        Ok(file) => Some(file),
        Err(error) => {
            warnings.push(format!("could not open {}: {error}", path.display()));
            None
        }
    }
}

/// Return the launch instant as `YYYYMMDD-HHMMSS`.
fn launch_stamp(now: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

/// Delete every launch log in `dir` but the newest [`KEEP_LAUNCH_LOGS`].
///
/// Return one warning per entry that could not be removed. A missing
/// directory has nothing to prune.
fn prune(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| is_launch_log(name))
        .collect();
    // A launch name holds a fixed-width timestamp, so name order is launch
    // order: the prune does not depend on the file system's mtime resolution.
    names.sort_unstable();
    let excess = names.len().saturating_sub(KEEP_LAUNCH_LOGS);
    names
        .drain(..excess)
        .filter_map(|name| remove_log(dir, &name))
        .collect()
}

/// Remove one log; return a warning when the entry refuses removal.
fn remove_log(dir: &Path, name: &str) -> Option<String> {
    let path = dir.join(name);
    match std::fs::remove_file(&path) {
        Ok(()) => None,
        Err(error) => Some(format!("could not prune {}: {error}", path.display())),
    }
}

/// Return whether `name` is one of this app's launch logs.
fn is_launch_log(name: &str) -> bool {
    name.starts_with(LOG_BASENAME) && name.ends_with(LOG_SUFFIX)
}
