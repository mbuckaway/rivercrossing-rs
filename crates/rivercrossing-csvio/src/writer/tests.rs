// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the atomic CSV swap.

use std::io::ErrorKind;
use std::path::Path;

use pretty_assertions::assert_eq;

use super::{write_bytes_atomic, write_bytes_atomic_with};
use crate::error::CsvIoError;

#[test]
fn test_writer_write_bytes_atomic_writes_the_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("roster.csv");

    write_bytes_atomic(&path, b"a,b\r\n")?;

    assert_eq!(std::fs::read(&path)?, b"a,b\r\n".to_vec());
    Ok(())
}

#[test]
fn test_writer_write_bytes_atomic_replaces_existing_bytes() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("roster.csv");
    std::fs::write(&path, b"old\r\n")?;

    write_bytes_atomic(&path, b"new\r\n")?;

    assert_eq!(std::fs::read(&path)?, b"new\r\n".to_vec());
    Ok(())
}

#[test]
fn test_writer_write_bytes_atomic_with_rename_failure_keeps_the_destination_and_no_temp()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("roster.csv");
    std::fs::write(&path, b"old\r\n")?;

    let result = write_bytes_atomic_with(&path, b"new\r\n", |_, _| {
        Err(std::io::Error::other("rename refused"))
    });

    assert!(matches!(result, Err(CsvIoError::Io(_))));
    assert_eq!(std::fs::read(&path)?, b"old\r\n".to_vec());
    assert!(!dir.path().join("roster.csv.tmp").exists());
    Ok(())
}

#[test]
fn test_writer_write_bytes_atomic_missing_parent_directory_returns_io() {
    let dir = tempfile::TempDir::new().expect("the temporary directory is created");
    let path = dir.path().join("missing").join("roster.csv");

    let error = write_bytes_atomic(&path, b"a,b\r\n").expect_err("the parent directory is absent");

    assert!(matches!(
        error,
        CsvIoError::Io(cause) if cause.kind() == ErrorKind::NotFound
    ));
}

#[test]
fn test_writer_write_bytes_atomic_path_without_file_name_returns_invalid_input() {
    let error = write_bytes_atomic(Path::new(""), b"a,b\r\n")
        .expect_err("an empty path carries no file name");

    assert!(matches!(
        error,
        CsvIoError::Io(cause) if cause.kind() == ErrorKind::InvalidInput
    ));
}
