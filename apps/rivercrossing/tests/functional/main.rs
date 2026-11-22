// SPDX-License-Identifier: GPL-3.0-only

//! The launch-path smoke test: open the empty shell window, then let the debug seam quit it.
//!
//! EPIC-1's one functional test. It spawns the real binary, so it needs a display (or
//! `xvfb-run`) and never belongs to the unit sweep.

use std::io;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// The `RIVERCROSSING_SMOKE_QUIT_AFTER_MS` value that makes the window quit by itself.
const QUIT_AFTER_MS: &str = "750";

/// The window must still be open this long after launch.
const SURVIVAL_WINDOW: Duration = Duration::from_millis(250);

/// The longest the test waits for the window to close and the process to exit.
const EXIT_TIMEOUT: Duration = Duration::from_secs(60);

/// The exit-wait poll interval.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Spawn the built binary with the debug quit seam armed and its stderr captured.
fn spawn_smoke_process() -> io::Result<Child> {
    Command::new(env!("CARGO_BIN_EXE_rivercrossing"))
        .env("RIVERCROSSING_SMOKE_QUIT_AFTER_MS", QUIT_AFTER_MS)
        .stderr(Stdio::piped())
        .spawn()
}

/// Wait for the child to exit, polling `try_wait` until the timeout, and return its output.
fn wait_for_exit(mut child: Child) -> io::Result<Output> {
    let deadline = Instant::now() + EXIT_TIMEOUT;
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        assert!(
            Instant::now() < deadline,
            "the smoke process did not exit within {} s",
            EXIT_TIMEOUT.as_secs()
        );
        thread::sleep(POLL_INTERVAL);
    }
}

#[test]
fn open_and_quit_smoke() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let mut child = spawn_smoke_process()?;

    // Act: a process that exits inside the survival window never opened a window.
    thread::sleep(SURVIVAL_WINDOW);
    let early_status = child.try_wait()?;
    let output = wait_for_exit(child)?;

    // Assert
    assert_eq!(early_status, None, "the process must survive launch");
    assert!(
        output.status.success(),
        "the smoke run must exit successfully, got {:?}",
        output.status
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked at"),
        "stderr shows a panic:\n{stderr}"
    );
    assert!(
        !stderr.contains("Error:"),
        "stderr shows an error:\n{stderr}"
    );
    Ok(())
}
