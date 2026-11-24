// SPDX-License-Identifier: GPL-3.0-only

//! The live control channel: the craft server, the project method table and the per-pass glue.

use std::fmt;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use craft::automation::args::ControlOptions;
use craft::automation::control::{ControlChannel, ControlRequest, Reply};
use craft::automation::screenshot::{self, Finished};
use craft::automation::server::{Listening, listen};
use craft::automation::synthetic;
use craft::egui::{Context, Rect, ViewportCommand, ViewportId, pos2, vec2};
use craft::ui::runtime::motion;
use craft::ui::runtime::windows::WindowHost;
use serde_json::{Map, Value, json};

use super::command::{ProbeOutcome, probe_command};
use super::path;
use super::selector::{Candidate, Selector, resolve};

/// The largest viewport side `ui.resize` accepts, in points.
const MAX_VIEWPORT_SIDE: f32 = 16_384.0;

/// How long to wait before the next frame while a screenshot is outstanding.
const CAPTURE_REPAINT: Duration = Duration::from_millis(16);

/// The control acquisition: the craft options and the token the server authenticates with.
#[derive(Debug)]
pub struct ControlConfig {
    /// The port and token-file acquisition; `port` is set whenever the channel starts.
    pub options: ControlOptions,
    /// The 64-hex server token.
    pub token: String,
}

/// The debug probe surface: the click count, the last echo and the input field.
#[derive(Debug, Default)]
pub(crate) struct ProbeState {
    /// How many times the probe button was clicked.
    pub(crate) clicks: u64,
    /// The value the probe label shows.
    pub(crate) echo: Option<String>,
    /// The probe text field's contents.
    pub(crate) text: String,
}

/// The per-pass values a dispatched request needs.
pub(crate) struct Pass<'a> {
    /// The debug probe state.
    pub probe: &'a mut ProbeState,
    /// The egui context of the running pass.
    pub ctx: &'a Context,
}

/// The live control server and the project method table it feeds.
pub struct Control {
    channel: ControlChannel,
    listening: Listening,
    next_token: u64,
    root: PathBuf,
    close: bool,
}

impl fmt::Debug for Control {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Control")
            .field("addr", &self.listening.addr)
            .field("next_token", &self.next_token)
            .field("root", &self.root)
            .field("close", &self.close)
            .finish_non_exhaustive()
    }
}

impl Control {
    /// Bind the loopback control server and create the screenshot write root.
    ///
    /// # Errors
    ///
    /// Return an error when no port is configured, the address cannot be bound, or the write root
    /// cannot be created.
    pub fn start(config: ControlConfig, ctx: &Context) -> Result<Self, String> {
        let port = config
            .options
            .port
            .ok_or_else(|| "no control port is configured".to_owned())?;
        let (requests, listening) = listen(port, config.token, WindowHost::of(ctx))
            .map_err(|error| format!("cannot listen on 127.0.0.1:{port}: {error}"))?;
        let root = path::write_root()
            .map_err(|error| format!("cannot create the control directory: {error}"))?;
        writeln!(
            std::io::stderr(),
            "control server listening on {}",
            listening.addr
        )
        .ok();
        Ok(Self {
            channel: ControlChannel::new(requests),
            listening,
            next_token: 1,
            root,
            close: false,
        })
    }

    /// Run the logic-phase steps of one pass: refresh capture geometry, settle finished captures,
    /// answer held access-tree requests, dispatch queued requests and close on `probe_quit`.
    pub(crate) fn logic(&mut self, probe: &mut ProbeState, ctx: &Context) {
        let host = WindowHost::of(ctx);
        self.channel.captures.note_windows(&host, None);
        let finished = self.channel.captures.drain(motion::now(), None);
        self.finish_captures(finished);
        self.channel.poll_access_trees(ctx);
        let mut pass = Pass { probe, ctx };
        while let Some(request) = self.channel.next_request() {
            let reply = self.dispatch(&request, &mut pass);
            self.channel.settle(
                request,
                reply,
                None,
                screenshot::settle_ms(ctx.global_style().animation_time),
            );
        }
        if self.channel.captures.any_pending() {
            ctx.request_repaint_after(CAPTURE_REPAINT);
        }
        if self.close {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// Run the UI-phase steps of one pass: settle capture passes, issue due captures and answer
    /// the input waiters.
    pub(crate) fn ui(&mut self, ctx: &Context) {
        let host = WindowHost::of(ctx);
        self.channel.captures.note_pass();
        let finished = self.channel.captures.issue(&host, motion::now(), None);
        self.finish_captures(finished);
        self.channel.poll_input(&host);
        if self.channel.captures.any_pending() {
            ctx.request_repaint_after(CAPTURE_REPAINT);
        }
    }

    /// Write each finished capture through the confined write root.
    fn finish_captures(&mut self, finished: Vec<Finished>) {
        let root = self.root.as_path();
        let mut write =
            |name: &str, bytes: &[u8]| path::write_screenshot(root, name, bytes).map(|_| ());
        self.channel.finish_captures(finished, &mut write);
    }

    /// Answer one request through the project method table.
    pub(crate) fn dispatch(&mut self, request: &ControlRequest, pass: &mut Pass<'_>) -> Reply {
        let ctx = pass.ctx;
        let host = WindowHost::of(ctx);
        match request.method.as_str() {
            "ui.accessTree" => Reply::access_tree(),
            "ui.command" => command_reply(&request.params, pass.probe, &mut self.close),
            "ui.screenshot" => self.screenshot_reply(&request.params),
            "ui.resize" => resize(&request.params, ctx),
            "ui.click" => click(&request.params, &host, ctx),
            "ui.type" => type_text(&request.params, &host, ctx),
            "ui.dialog.set" | "ui.dialog.confirm" | "ui.dialog.cancel" => {
                Reply::from_result(Err(format!("no hosted dialog for `{}`", request.method)))
            }
            other => inject(other, &request.params, &host),
        }
    }

    /// Allocate the next screenshot token and admit a capture of the root viewport.
    fn screenshot_reply(&mut self, params: &Value) -> Reply {
        let path = match params.get("path") {
            None => None,
            Some(Value::String(path)) => Some(path.clone()),
            Some(_) => return Reply::from_result(Err("`path` must be a string".to_owned())),
        };
        let token = self.next_token;
        self.next_token = self.next_token.saturating_add(1);
        Reply::screenshot(token, path, ViewportId::ROOT)
    }
}

/// Answer a `ui.command` probe.
fn command_reply(params: &Value, probe: &mut ProbeState, close: &mut bool) -> Reply {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Reply::from_result(Err("ui.command needs a string `name`".to_owned()));
    };
    let missing = Value::Null;
    let command_params = params.get("params").unwrap_or(&missing);
    match probe_command(name, command_params) {
        Ok(ProbeOutcome::Echo(value)) => Reply::from_result(Ok(json!({"echo": value}))),
        Ok(ProbeOutcome::EchoField) => {
            let text = probe.text.clone();
            probe.echo = Some(text.clone());
            Reply::from_result(Ok(json!({"echo": text})))
        }
        Ok(ProbeOutcome::Quit(value)) => {
            *close = true;
            Reply::from_result(Ok(value))
        }
        Err(error) => Reply::from_result(Err(error)),
    }
}

/// Resize the root viewport from `{width, height}` in points.
fn resize(params: &Value, ctx: &Context) -> Reply {
    let Some(object) = params.as_object() else {
        return Reply::from_result(Err("params must be an object".to_owned()));
    };
    let size = match (dimension(object, "width"), dimension(object, "height")) {
        (Ok(width), Ok(height)) => vec2(width, height),
        (Err(error), _) | (_, Err(error)) => return Reply::from_result(Err(error)),
    };
    ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
    Reply::from_result(Ok(Value::Null))
}

/// One positive, finite viewport side from `object`.
fn dimension(object: &Map<String, Value>, key: &str) -> Result<f32, String> {
    let value = object.get(key).ok_or_else(|| format!("missing `{key}`"))?;
    let number: f32 =
        serde_json::from_value(value.clone()).map_err(|_| format!("`{key}` must be a number"))?;
    if number.is_finite() && (1.0..=MAX_VIEWPORT_SIDE).contains(&number) {
        Ok(number)
    } else {
        Err(format!("`{key}` must be between 1 and {MAX_VIEWPORT_SIDE}"))
    }
}

/// Click the centre of the node `selector` names.
fn click(params: &Value, host: &WindowHost, ctx: &Context) -> Reply {
    let bounds = match selector_bounds(params, ctx) {
        Ok(bounds) => bounds,
        Err(error) => return Reply::from_result(Err(error)),
    };
    let center = bounds.center();
    inject("ui.click", &json!({"x": center.x, "y": center.y}), host)
}

/// Type `text` into the node `selector` names, clicking it first to focus it.
fn type_text(params: &Value, host: &WindowHost, ctx: &Context) -> Reply {
    let Some(text) = params.get("text").and_then(Value::as_str) else {
        return Reply::from_result(Err("ui.type needs a string `text`".to_owned()));
    };
    if params.get("selector").is_some() {
        let bounds = match selector_bounds(params, ctx) {
            Ok(bounds) => bounds,
            Err(error) => return Reply::from_result(Err(error)),
        };
        let center = bounds.center();
        let clicked = inject("ui.click", &json!({"x": center.x, "y": center.y}), host);
        if !matches!(&clicked, Reply::AfterInput) {
            return clicked;
        }
    }
    inject("ui.type", &json!({"text": text}), host)
}

/// Resolve the request's `selector` against the root window's settled access tree.
fn selector_bounds(params: &Value, ctx: &Context) -> Result<Rect, String> {
    let value = params
        .get("selector")
        .ok_or_else(|| "missing `selector`".to_owned())?;
    let selector: Selector =
        serde_json::from_value(value.clone()).map_err(|error| format!("bad selector: {error}"))?;
    resolve(&selector, &candidates(ctx))
}

/// The access-tree nodes of the root window that carry bounds.
#[expect(
    clippy::cast_possible_truncation,
    reason = "AccessKit bounds are egui points, which are f32"
)]
fn candidates(ctx: &Context) -> Vec<Candidate> {
    let Some(update) = WindowHost::of(ctx).access_tree(ViewportId::ROOT) else {
        return Vec::new();
    };
    update
        .nodes
        .iter()
        .filter_map(|(_, node)| {
            let bounds = node.bounds()?;
            Some(Candidate {
                role: format!("{:?}", node.role()),
                label: node.label().map(str::to_owned),
                bounds: Rect::from_min_max(
                    pos2(bounds.x0 as f32, bounds.y0 as f32),
                    pos2(bounds.x1 as f32, bounds.y1 as f32),
                ),
            })
        })
        .collect()
}

/// Queue one craft synthetic input method and settle the reply after it has been processed.
fn inject(method: &str, params: &Value, host: &WindowHost) -> Reply {
    match synthetic::input_method(method, params, host, ViewportId::ROOT, &|_, _| None) {
        Some(Ok(())) => Reply::AfterInput,
        Some(Err(error)) => Reply::from_result(Err(error)),
        None => Reply::from_result(Err(format!("unknown method `{method}`"))),
    }
}
