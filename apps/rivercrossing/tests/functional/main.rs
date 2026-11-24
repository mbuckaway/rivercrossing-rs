// SPDX-License-Identifier: GPL-3.0-only

//! The launch-path smoke tests: the empty shell window and the live automation channel.
//!
//! EPIC-1's launch smoke and EPIC-10's control-channel smoke. Both spawn the real binary, so
//! they need a display (or `xvfb-run`) and never belong to the unit sweep.

use std::io::{self, BufRead, BufReader};
use std::net::SocketAddr;
use std::path::Path;
use std::process::{Child, ChildStderr, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use craft::automation::client::ControlClient;
use serde_json::{Value, json};

/// The `RIVERCROSSING_SMOKE_QUIT_AFTER_MS` value that makes the window quit by itself.
const QUIT_AFTER_MS: &str = "750";

/// The window must still be open this long after launch.
const SURVIVAL_WINDOW: Duration = Duration::from_millis(250);

/// The longest the test waits for the window to close and the process to exit.
const EXIT_TIMEOUT: Duration = Duration::from_secs(60);

/// The exit-wait poll interval.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// The fixed 64-hex token the control smoke authenticates with; the child never generates one.
const CONTROL_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// The stderr line the app prints once the control server listens.
const LISTENING_PREFIX: &str = "control server listening on ";

/// The longest the control smoke waits for the listening line.
const LISTEN_TIMEOUT: Duration = Duration::from_secs(10);

/// The longest the control smoke waits for the child to exit after `probe_quit`.
const CONTROL_EXIT_TIMEOUT: Duration = Duration::from_secs(30);

/// A smoke step's result.
type SmokeResult = Result<(), Box<dyn std::error::Error>>;

/// Spawn the built binary with the debug quit seam armed and its stderr captured.
fn spawn_smoke_process() -> io::Result<Child> {
    Command::new(env!("CARGO_BIN_EXE_rivercrossing"))
        .env("RIVERCROSSING_SMOKE_QUIT_AFTER_MS", QUIT_AFTER_MS)
        .stderr(Stdio::piped())
        .spawn()
}

/// Wait for the child to exit, polling `try_wait` until the timeout, and return its output.
fn wait_for_exit(mut child: Child) -> io::Result<std::process::Output> {
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

/// Spawn the built binary with the control channel armed and every environment fallback removed.
///
/// The working directory is the workspace root, so the screenshot write root is the repository's
/// own `target/control`.
fn spawn_control_process(workspace: &Path) -> io::Result<Child> {
    Command::new(env!("CARGO_BIN_EXE_rivercrossing"))
        .args(["--control", "0", "--control-token", CONTROL_TOKEN])
        .current_dir(workspace)
        .env_remove("RIVERCROSSING_SMOKE_QUIT_AFTER_MS")
        .env_remove("RIVERCROSSING_CONTROL_PORT")
        .env_remove("RIVERCROSSING_CONTROL_TOKEN")
        .env_remove("RIVERCROSSING_CONTROL_TOKEN_FILE")
        .stderr(Stdio::piped())
        .spawn()
}

/// A spawned control app whose child is killed and reaped if the smoke fails before it exits.
struct ControlProcess {
    child: Option<Child>,
}

impl ControlProcess {
    /// Spawn the control app in `workspace`.
    fn spawn(workspace: &Path) -> io::Result<Self> {
        spawn_control_process(workspace).map(|child| Self { child: Some(child) })
    }

    /// Take the piped stderr so a reader thread can drain it.
    fn take_stderr(&mut self) -> io::Result<ChildStderr> {
        self.child
            .as_mut()
            .and_then(|child| child.stderr.take())
            .ok_or_else(|| io::Error::other("the control process has no piped stderr"))
    }

    /// Wait for the child to exit, killing and reaping it when the timeout passes.
    fn wait_for_exit(mut self) -> io::Result<ExitStatus> {
        let Some(mut child) = self.child.take() else {
            return Err(io::Error::other("the control process was already reaped"));
        };
        let deadline = Instant::now() + CONTROL_EXIT_TIMEOUT;
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                child.kill().ok();
                child.wait().ok();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the control process did not exit after probe_quit",
                ));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }
}

impl Drop for ControlProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            child.kill().ok();
            child.wait().ok();
        }
    }
}

/// Drain the child's stderr on a reader thread, forwarding every line to the returned receiver.
fn spawn_stderr_reader(stderr: ChildStderr) -> Receiver<String> {
    let (lines, received) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                break;
            }
        }
    });
    received
}

/// Wait for the listening line and return the bound address it names.
fn wait_for_address(lines: &Receiver<String>) -> Result<SocketAddr, String> {
    let deadline = Instant::now() + LISTEN_TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let line = lines.recv_timeout(left).map_err(|_| {
            format!(
                "the app did not print `{LISTENING_PREFIX}<addr>` within {} s",
                LISTEN_TIMEOUT.as_secs()
            )
        })?;
        if let Some(address) = line
            .strip_prefix(LISTENING_PREFIX)
            .and_then(|rest| rest.trim().parse().ok())
        {
            return Ok(address);
        }
    }
}

/// Return the node array of a `ui.accessTree` result, or an empty slice when the shape is wrong.
fn tree_nodes(tree: &Value) -> &[Value] {
    tree.get("nodes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// Assert the settled root tree has a root and nodes.
fn assert_access_tree(client: &mut ControlClient) -> SmokeResult {
    let tree = client.call("ui.accessTree", json!({}))?;
    assert!(
        !tree_nodes(&tree).is_empty(),
        "the settled access tree has nodes: {tree}"
    );
    assert!(
        tree.get("root").is_some_and(|root| !root.is_null()),
        "the settled access tree has a root: {tree}"
    );
    Ok(())
}

/// Assert `probe_echo` echoes its parameters.
fn assert_command_echo(client: &mut ControlClient) -> SmokeResult {
    let echo = client.call(
        "ui.command",
        json!({"name": "probe_echo", "params": {"x": 1}}),
    )?;
    assert_eq!(echo, json!({"echo": {"x": 1}}));
    Ok(())
}

/// Assert a selector click and a selector type reach the probe surface.
fn assert_click_and_type(client: &mut ControlClient) -> SmokeResult {
    let clicked = client.call("ui.click", json!({"selector": {"action": "probe_echo"}}))?;
    assert_eq!(clicked, Value::Null);

    let typed = client.call(
        "ui.type",
        json!({"selector": {"action": "probe_input"}, "text": "abc"}),
    )?;
    assert_eq!(typed, Value::Null);
    let field = client.call("ui.command", json!({"name": "probe_echo"}))?;
    assert_eq!(field, json!({"echo": "abc"}));
    let typed_tree = client.call("ui.accessTree", json!({}))?;
    assert!(
        tree_nodes(&typed_tree)
            .iter()
            .any(|node| node.get("value") == Some(&json!("abc"))),
        "the settled tree shows the typed value: {typed_tree}"
    );
    Ok(())
}

/// Assert a PNG lands under the control root and a parent escape is refused.
fn assert_screenshot(client: &mut ControlClient, workspace: &Path) -> SmokeResult {
    let shot = client.call("ui.screenshot", json!({"path": "smoke.png"}))?;
    assert!(
        shot.get("width")
            .and_then(Value::as_u64)
            .is_some_and(|width| width > 0),
        "the screenshot reply reports a width: {shot}"
    );
    let png = std::fs::read(workspace.join("target/control/smoke.png"))?;
    assert!(
        png.starts_with(b"\x89PNG\r\n\x1a\n"),
        "smoke.png is a PNG ({} bytes)",
        png.len()
    );

    let escaped = client.call("ui.screenshot", json!({"path": "../escape.png"}));
    let refusal = escaped
        .err()
        .ok_or("a parent-escape screenshot path is refused")?;
    assert!(
        refusal.contains("must stay inside the control directory"),
        "the refusal names the escape: {refusal}"
    );
    assert!(
        !workspace.join("target/escape.png").exists(),
        "no file is written outside the control root"
    );
    Ok(())
}

/// Assert the window survives a resize and still answers.
fn assert_resize(client: &mut ControlClient) -> SmokeResult {
    let resized = client.call("ui.resize", json!({"width": 900, "height": 700}))?;
    assert_eq!(resized, Value::Null);
    let after_resize = client.call("ui.accessTree", json!({}))?;
    assert!(
        !tree_nodes(&after_resize).is_empty(),
        "the window survives a resize: {after_resize}"
    );
    Ok(())
}

/// Assert `probe_quit` replies and the process exits successfully.
fn assert_quit(client: &mut ControlClient, app: ControlProcess) -> SmokeResult {
    let bye = client.call("ui.command", json!({"name": "probe_quit"}))?;
    assert_eq!(bye, json!({"bye": true}));
    let status = app.wait_for_exit()?;
    assert!(status.success(), "the control run exits 0, got {status:?}");
    Ok(())
}

#[test]
fn control_channel_smoke() -> SmokeResult {
    // Arrange: the real binary, the fixed token and the repository root as its working directory.
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut app = ControlProcess::spawn(&workspace)?;
    let lines = spawn_stderr_reader(app.take_stderr()?);
    let address = wait_for_address(&lines)?;
    let mut client = ControlClient::connect(address, CONTROL_TOKEN)?;

    // Act and assert: one live-channel step per helper.
    assert_access_tree(&mut client)?;
    assert_command_echo(&mut client)?;
    assert_click_and_type(&mut client)?;
    assert_screenshot(&mut client, &workspace)?;
    assert_resize(&mut client)?;
    assert_quit(&mut client, app)
}
