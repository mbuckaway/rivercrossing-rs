// SPDX-License-Identifier: GPL-3.0-only

//! The launch-path smoke tests: the empty shell window and the live automation channel.
//!
//! EPIC-1's launch smoke and EPIC-10's control-channel smoke. Both spawn the real binary, so
//! they need a display (or `xvfb-run`) and never belong to the unit sweep.

use std::io::{self, BufRead, BufReader, Write};
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

/// The longest the control smoke waits for the child to exit after `app_quit`.
const CONTROL_EXIT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the control smoke lets the first frame finish before its first request.
///
/// craft-libs' control server takes the context's pass state to read the frame clock when it wakes
/// the window (`craft_automation::server::serve_connection`), which panics if the app is inside a
/// pass. The first pass builds the window and the GPU device; once it ends the app idles between
/// passes, so a request is safe.
const SETTLE_DELAY: Duration = Duration::from_secs(2);

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
    // Keep the smoke's settings, logs and database under the workspace, in fresh directories.
    std::fs::create_dir_all(workspace.join("target/control/config"))?;
    std::fs::create_dir_all(workspace.join("target/control/data"))?;
    Command::new(env!("CARGO_BIN_EXE_rivercrossing"))
        .args(["--control", "0", "--control-token", CONTROL_TOKEN])
        .current_dir(workspace)
        .env("XDG_CONFIG_HOME", workspace.join("target/control/config"))
        .env("XDG_DATA_HOME", workspace.join("target/control/data"))
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
                    "the control process did not exit after app_quit",
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

/// Drain the child's stderr on a reader thread, forwarding every line to the returned receiver and
/// appending it to *log* for a failing run's artifact.
fn spawn_stderr_reader(stderr: ChildStderr, log: &Path) -> Receiver<String> {
    let (lines, received) = mpsc::channel();
    let log = log.to_path_buf();
    thread::spawn(move || {
        let mut sink = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .ok();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(file) = sink.as_mut() {
                writeln!(file, "{line}").ok();
            }
            if lines.send(line).is_err() {
                break;
            }
        }
    });
    received
}

/// Name the smoke step that failed.
fn step(name: &str, result: SmokeResult) -> SmokeResult {
    result.map_err(|error| format!("{name}: {error}").into())
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

/// Return whether any tree node's name or value equals *text*.
fn tree_mentions(tree: &Value, text: &str) -> bool {
    tree_nodes(tree).iter().any(|node| {
        node.get("name").and_then(Value::as_str) == Some(text)
            || node.get("value").and_then(Value::as_str) == Some(text)
    })
}

/// Assert the disabled console controls answer a click and a type without failing.
fn assert_console_click_and_type(client: &mut ControlClient) -> SmokeResult {
    let clicked = client.call(
        "ui.click",
        json!({"selector": {"role": "Button", "label": "Record"}}),
    )?;
    assert_eq!(clicked, Value::Null);

    // The entry is closed with no ride, so the typed text reaches nothing and nothing breaks.
    let typed = client.call(
        "ui.type",
        json!({"selector": {"role": "TextInput", "label": "Plate"}, "text": "abc"}),
    )?;
    assert_eq!(typed, Value::Null);
    let tree = client.call("ui.accessTree", json!({}))?;
    assert!(
        !tree_nodes(&tree).is_empty(),
        "the console still renders after the input: {tree}"
    );
    Ok(())
}

/// Fill the remaining setup fields, confirm the form and wait for the ride to load.
fn assert_setup_commits_and_loads(client: &mut ControlClient) -> SmokeResult {
    for (field, value) in [
        ("venue_input", "Smoke Venue"),
        ("organizer_input", "Smoke Club"),
        ("scorer_input", "Smoke Scorer"),
    ] {
        let reply = client.call("ui.set", json!({"field": field, "value": value}))?;
        assert_eq!(reply, json!({"ok": true}));
    }
    let confirmed = client.call("ui.dialog.confirm", json!({}))?;
    assert_eq!(
        confirmed.get("event"),
        Some(&json!("confirmed")),
        "the setup form commits: {confirmed}"
    );
    for _ in 0..100 {
        let tree = client.call("ui.accessTree", json!({}))?;
        if tree_mentions(&tree, "smoke ride") {
            return Ok(());
        }
    }
    Err("the ride did not load within 100 access-tree polls".into())
}

/// Assert the ride-gated rider editor opens and `ui.dialog.cancel` closes it.
fn assert_rider_editor_round_trip(client: &mut ControlClient) -> SmokeResult {
    let opened = client.call("ui.command", json!({"name": "mi_rider_editor"}))?;
    assert_eq!(opened, json!({"ok": true}));
    let tree = client.call("ui.accessTree", json!({}))?;
    assert!(
        tree_mentions(&tree, "Rider Editor"),
        "the rider editor is open: {tree}"
    );
    let cancelled = client.call("ui.dialog.cancel", json!({}))?;
    assert_eq!(
        cancelled.get("event"),
        Some(&json!("cancelled")),
        "the editor reports its cancel: {cancelled}"
    );
    let after = client.call("ui.accessTree", json!({}))?;
    assert!(
        !tree_mentions(&after, "Rider Editor"),
        "the editor closed: {after}"
    );
    Ok(())
}

/// Assert the Preferences row opens the Settings dialog and `ui.dialog.cancel` closes it.
fn assert_settings_round_trip(client: &mut ControlClient) -> SmokeResult {
    let opened = client.call("ui.command", json!({"name": "wxID_PREFERENCES"}))?;
    assert_eq!(opened, json!({"ok": true}));
    let tree = client.call("ui.accessTree", json!({}))?;
    assert!(
        tree_mentions(&tree, "Settings"),
        "the settings dialog is open: {tree}"
    );
    assert!(
        tree_mentions(&tree, "Appearance"),
        "the settings form shows its appearance section: {tree}"
    );
    let cancelled = client.call("ui.dialog.cancel", json!({}))?;
    assert_eq!(
        cancelled.get("event"),
        Some(&json!("cancelled")),
        "the settings dialog reports its cancel: {cancelled}"
    );
    let after = client.call("ui.accessTree", json!({}))?;
    assert!(
        !tree_mentions(&after, "Appearance"),
        "the settings dialog closed: {after}"
    );
    Ok(())
}

/// Assert `mi_standings` opens the results window and `ui.dialog.cancel` closes it.
fn assert_results_round_trip(client: &mut ControlClient) -> SmokeResult {
    let opened = client.call("ui.command", json!({"name": "mi_standings"}))?;
    assert_eq!(opened, json!({"ok": true}));
    let tree = client.call("ui.accessTree", json!({}))?;
    assert!(
        tree_mentions(&tree, "Standings"),
        "the results window is open: {tree}"
    );
    assert!(
        tree_mentions(&tree, "Place"),
        "the results board shows its columns: {tree}"
    );
    let cancelled = client.call("ui.dialog.cancel", json!({}))?;
    assert_eq!(
        cancelled.get("event"),
        Some(&json!("cancelled")),
        "the results window reports its cancel: {cancelled}"
    );
    let after = client.call("ui.accessTree", json!({}))?;
    assert!(
        !tree_mentions(&after, "Standings"),
        "the results window closed: {after}"
    );
    Ok(())
}

/// Assert the Exit row raises the native confirmation, and `ui.dialog.cancel` dismisses it.
fn assert_native_confirm_cancels(client: &mut ControlClient) -> SmokeResult {
    let opened = client.call("ui.command", json!({"name": "wxID_EXIT"}))?;
    assert_eq!(opened, json!({"ok": true}));
    let tree = client.call("ui.accessTree", json!({}))?;
    assert!(
        tree_mentions(&tree, "Quit RiverCrossing?"),
        "the quit confirmation is open: {tree}"
    );
    let cancelled = client.call("ui.dialog.cancel", json!({}))?;
    assert_eq!(
        cancelled.get("event"),
        Some(&json!("cancelled")),
        "the confirmation reports its cancel: {cancelled}"
    );
    let after = client.call("ui.accessTree", json!({}))?;
    assert!(
        !tree_mentions(&after, "Quit RiverCrossing?"),
        "the confirmation closed: {after}"
    );
    Ok(())
}

/// Assert `mi_new_ride` answers ok and the settled tree shows the setup dialog.
fn assert_command_opens_setup(client: &mut ControlClient) -> SmokeResult {
    let opened = client.call("ui.command", json!({"name": "mi_new_ride"}))?;
    assert_eq!(opened, json!({"ok": true}));
    let tree = client.call("ui.accessTree", json!({}))?;
    assert!(
        tree_mentions(&tree, "Ride Setup"),
        "the setup dialog is open: {tree}"
    );
    Ok(())
}

/// Assert a selector click and a selector type reach the setup form's name field.
fn assert_click_and_type(client: &mut ControlClient) -> SmokeResult {
    let selector = json!({"role": "TextInput", "label": "Ride name"});
    let clicked = client.call("ui.click", json!({"selector": selector}))?;
    assert_eq!(clicked, Value::Null);

    let typed = client.call(
        "ui.type",
        json!({"selector": selector, "text": "smoke ride"}),
    )?;
    assert_eq!(typed, Value::Null);
    let tree = client.call("ui.accessTree", json!({}))?;
    assert!(
        tree_nodes(&tree)
            .iter()
            .any(|node| node.get("value") == Some(&json!("smoke ride"))),
        "the settled tree shows the typed name: {tree}"
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

/// Assert `app_quit` replies and the process exits successfully.
fn assert_quit(client: &mut ControlClient, app: ControlProcess) -> SmokeResult {
    let bye = client.call("ui.command", json!({"name": "app_quit"}))?;
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
    let lines = spawn_stderr_reader(
        app.take_stderr()?,
        &workspace.join("target/control/control-smoke.log"),
    );
    let address = wait_for_address(&lines)?;
    thread::sleep(SETTLE_DELAY);
    let mut client = ControlClient::connect(address, CONTROL_TOKEN)?;

    // Act and assert: one live-channel step per helper.
    step("access tree", assert_access_tree(&mut client))?;
    step(
        "console click and type",
        assert_console_click_and_type(&mut client),
    )?;
    step(
        "exit confirmation",
        assert_native_confirm_cancels(&mut client),
    )?;
    step("new ride command", assert_command_opens_setup(&mut client))?;
    step("setup click and type", assert_click_and_type(&mut client))?;
    step("setup commits", assert_setup_commits_and_loads(&mut client))?;
    step(
        "rider editor round trip",
        assert_rider_editor_round_trip(&mut client),
    )?;
    step(
        "settings round trip",
        assert_settings_round_trip(&mut client),
    )?;
    step("results round trip", assert_results_round_trip(&mut client))?;
    step("screenshot", assert_screenshot(&mut client, &workspace))?;
    step("resize", assert_resize(&mut client))?;
    step("app_quit", assert_quit(&mut client, app))
}
