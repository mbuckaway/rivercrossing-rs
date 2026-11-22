// SPDX-License-Identifier: GPL-3.0-only

//! The workspace maintenance tasks, run as `cargo xtask <task>`.

mod layers;

use std::io::Write as _;
use std::process::{Command, ExitCode, Stdio};

/// The prefix of every message that this tool writes.
const PREFIX: &str = "layers: ";

/// The usage line of the tool.
const USAGE: &str = "usage: cargo xtask layers";

/// The exit code of a failed check.
const FAILURE: u8 = 1;

/// The exit code of an unknown subcommand.
const USAGE_ERROR: u8 = 2;

/// Run the subcommand that the arguments name.
fn main() -> Result<ExitCode, std::io::Error> {
    let mut arguments = std::env::args().skip(1);
    let supported = arguments.next().as_deref() == Some("layers") && arguments.next().is_none();
    if !supported {
        writeln!(std::io::stderr(), "{USAGE}")?;
        return Ok(ExitCode::from(USAGE_ERROR));
    }
    run_layers()
}

/// Run the layering check and return its exit code.
fn run_layers() -> Result<ExitCode, std::io::Error> {
    let metadata_json = match cargo_metadata_json() {
        Ok(metadata_json) => metadata_json,
        Err(error) => return fail(&error.to_string()),
    };

    let violations = match layers::violations(&metadata_json) {
        Ok(violations) => violations,
        Err(error) => return fail(&error.to_string()),
    };

    if violations.is_empty() {
        writeln!(std::io::stdout(), "{PREFIX}OK")?;
        return Ok(ExitCode::SUCCESS);
    }

    for violation in &violations {
        writeln!(std::io::stderr(), "{PREFIX}{violation}")?;
    }
    Ok(ExitCode::from(FAILURE))
}

/// Write one prefixed message to stderr and return the failure code.
fn fail(message: &str) -> Result<ExitCode, std::io::Error> {
    writeln!(std::io::stderr(), "{PREFIX}{message}")?;
    Ok(ExitCode::from(FAILURE))
}

/// Return the `cargo metadata --format-version 1` output of this workspace.
fn cargo_metadata_json() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .stderr(Stdio::inherit())
        .output()?;
    if !output.status.success() {
        return Err(format!("cargo metadata failed with {}", output.status).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}
