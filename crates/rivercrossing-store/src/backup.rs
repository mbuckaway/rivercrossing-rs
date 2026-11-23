// SPDX-License-Identifier: GPL-3.0-only

//! Backup run, restore and the hourly state machine.
//!
//! A backup lives in a sibling directory named `<db file name>.backups`, one
//! file per run named `<stem>.<YYYYmmdd-HHMMSS-ffffff>.db` from the injected
//! UTC clock. The fixed-width timestamp makes a file-name sort a chronological
//! sort, so rotation needs no metadata. A WAL database is checkpointed through
//! a short-lived second connection first, so the copied main file alone is a
//! complete snapshot.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rivercrossing_ride::{Clock, Timestamp};
use rusqlite::Connection;

use crate::error::StoreError;

#[cfg(test)]
mod tests;

/// The default number of backups to keep (R-54).
pub const DEFAULT_KEEP: usize = 20;

/// The suffix every backup file carries.
const BACKUP_SUFFIX: &str = ".db";

/// The suffix of the sibling backup directory.
const DIR_SUFFIX: &str = ".backups";

/// The number of seconds in one hour.
const SECONDS_PER_HOUR: i64 = 3_600;

/// Return the sibling backup directory for `db_path`.
///
/// Backups of `/path/rides.db` live in `/path/rides.db.backups`. The
/// directory is returned whether or not it exists yet.
pub(crate) fn backup_dir_for(db_path: &Path) -> PathBuf {
    let name = db_path.file_name().unwrap_or(db_path.as_os_str());
    let directory_name = format!("{}{DIR_SUFFIX}", name.to_string_lossy());
    db_path.parent().unwrap_or(db_path).join(directory_name)
}

/// Copy *path* to a timestamped backup, then prune to *keep* files.
///
/// The WAL is checkpointed through a second connection before the copy, so
/// the copied main file holds every committed page.
///
/// # Errors
///
/// Return [`StoreError::MissingDatabase`] when *path* is not a file,
/// [`StoreError::KeepOutOfRange`] when *keep* is below one, and
/// [`StoreError::CheckpointBusy`] when another connection holds a
/// transaction that the checkpoint cannot fold in (no file is written in
/// that case). A file-system failure surfaces as [`StoreError::Io`].
pub fn run(path: &Path, keep: usize, clock: &dyn Clock) -> Result<PathBuf, StoreError> {
    if !path.is_file() {
        return Err(StoreError::MissingDatabase {
            path: path.to_path_buf(),
        });
    }
    if keep < 1 {
        return Err(StoreError::KeepOutOfRange { keep });
    }
    checkpoint(path)?;
    let directory = backup_dir_for(path);
    std::fs::create_dir_all(&directory)?;
    let target = directory.join(format!(
        "{}.{}{BACKUP_SUFFIX}",
        backup_stem(path),
        backup_stamp(clock.now())
    ));
    std::fs::copy(path, &target)?;
    sync_file(&target)?;
    prune(&directory, keep)?;
    Ok(target)
}

/// Copy backup *src* back over the live path *dst*.
///
/// Removes stale `-wal` and `-shm` sidecars beside *dst*, so a fresh main
/// file is never paired with a WAL written against the old one. The live
/// store must be closed when restoring.
///
/// # Errors
///
/// Return [`StoreError::MissingBackupSource`] when *src* is not a file, and
/// [`StoreError::Io`] when the copy or the sidecar removal fails.
pub fn restore(src: &Path, dst: &Path) -> Result<(), StoreError> {
    if !src.is_file() {
        return Err(StoreError::MissingBackupSource {
            path: src.to_path_buf(),
        });
    }
    std::fs::copy(src, dst)?;
    sync_file(dst)?;
    remove_sidecars(dst)
}

/// The hourly scheduler that a caller ticks from its timer (R-54).
///
/// The first tick only seeds the hour. A later tick whose truncated UTC hour
/// is above the stored one runs the runner exactly once, and advances the
/// bucket before the runner, so a runner that fails does not run twice for
/// the same hour.
pub struct HourlyBackup<C: Clock> {
    runner: Box<dyn FnMut() -> Result<PathBuf, StoreError>>,
    clock: C,
    last_hour: Option<i64>,
}

impl<C: Clock> std::fmt::Debug for HourlyBackup<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HourlyBackup")
            .field("last_hour", &self.last_hour)
            .finish_non_exhaustive()
    }
}

impl<C: Clock> HourlyBackup<C> {
    /// Check the clock and run the backup at most once per new hour.
    ///
    /// # Errors
    ///
    /// Return the runner's error after the hour bucket has advanced.
    pub fn tick(&mut self) -> Result<bool, StoreError> {
        let hour = self
            .clock
            .now()
            .unix_timestamp()
            .div_euclid(SECONDS_PER_HOUR);
        match self.last_hour {
            None => {
                self.last_hour = Some(hour);
                Ok(false)
            }
            Some(last) if hour <= last => Ok(false),
            Some(_) => {
                self.last_hour = Some(hour);
                (self.runner)()?;
                Ok(true)
            }
        }
    }
}

/// Return an [`HourlyBackup`] for the caller to tick from its timer.
pub fn schedule_hourly<C: Clock>(
    runner: impl FnMut() -> Result<PathBuf, StoreError> + 'static,
    clock: C,
) -> HourlyBackup<C> {
    HourlyBackup {
        runner: Box::new(runner),
        clock,
        last_hour: None,
    }
}

/// Fold a WAL database's committed pages into its main file.
///
/// # Errors
///
/// Return [`StoreError::CheckpointBusy`] when the checkpoint leaves frames
/// behind, and [`StoreError::Sqlite`] when the connection or the PRAGMA
/// fails.
fn checkpoint(path: &Path) -> Result<(), StoreError> {
    let conn = Connection::open(path)?;
    let (busy, _log, _checkpointed) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
            ))
        })?;
    if busy == 0 {
        Ok(())
    } else {
        Err(StoreError::CheckpointBusy { frames: busy })
    }
}

/// Return the file stem that names the backup files of *path*.
fn backup_stem(path: &Path) -> String {
    path.file_stem()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// Render *now* in the fixed-width backup file-name format.
fn backup_stamp(now: Timestamp) -> String {
    let month = u8::from(now.month());
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}-{:06}",
        now.year(),
        month,
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        now.microsecond()
    )
}

/// Force *path*'s bytes to disk, so a crash leaves no half-copy.
fn sync_file(path: &Path) -> Result<(), StoreError> {
    std::fs::File::open(path)?.sync_all()?;
    Ok(())
}

/// Delete all but the newest *keep* backup files in *directory*.
fn prune(directory: &Path, keep: usize) -> Result<(), StoreError> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == OsStr::new("db"))
        {
            names.push(path);
        }
    }
    names.sort_unstable();
    names.reverse();
    for old in names.into_iter().skip(keep) {
        std::fs::remove_file(old)?;
    }
    Ok(())
}

/// Remove the stale `-wal` and `-shm` sidecars beside *dst*.
fn remove_sidecars(dst: &Path) -> Result<(), StoreError> {
    let parent = dst.parent().unwrap_or(dst);
    let name = dst.file_name().unwrap_or(dst.as_os_str()).to_string_lossy();
    for suffix in ["-wal", "-shm"] {
        let sidecar = parent.join(format!("{name}{suffix}"));
        if sidecar.exists() {
            std::fs::remove_file(sidecar)?;
        }
    }
    Ok(())
}
