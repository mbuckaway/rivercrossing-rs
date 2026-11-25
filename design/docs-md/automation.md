# RiverCrossing-rs — UI automation & Python-parity harness

Decision record and mechanism contract for **EPIC-10 (UI automation & parity)**. It fixes how the
Rust UI is driven, inspected and compared to the Python app before EPIC-12 builds the screens.
Architecture: [architecture](architecture.md) §12. Executable matrix: [test plan](test-plan.md).

## 1 · Context — why automation before the UI

- **No executable parity oracle exists.** The Python functional suite was deleted on 2026-09-12
  (R-74); exactly one functional test survives (the open/quit smoke). Without a replacement, the
  port's parity with the Python app can only be eyeballed.
- **EPIC-12 builds every screen.** Every screen and its dialogs are about to be authored from
  `design/screenshots/` and the §15b frozen-name registry. Without a test contract each screen
  would be verified only by unit tests on its presenter plus manual inspection — built blind.
- **EPIC-10 delivers the mechanism, not the screens.** An in-process semantic harness,
  deterministic offscreen snapshots, a live token-authenticated control channel, and the scenario
  schema the Python oracle speaks at EPIC-12 exit. The shell and a debug probe surface are the
  only UI exercised here.
- **Deliberately no new crate, no new binary, no cargo feature.** The model has one consumer and
  the cross-language contract is JSON, not Rust types (SIMPLECODE Rules 1/3/4/6). The runner is
  `cargo nextest`; the live channel rides the existing app binary.
- **If EPIC-10 is dropped**, EPIC-12 is still buildable, but there is no agent-drivable surface and
  no parity evidence until one is retrofitted.

## 2 · Decision — Rune evaluated and rejected

Rune (`anomalyco/rune`) is a **full-stack UI framework** — own renderer, text, layout and
windowing — not an automation library. Adopting it would mean rewriting `rivercrossing-ui-egui`
and dropping the craft shell/theme/settings/dialogs/menu/keymap/i18n stack.

| Finding | Evidence (retrieved 2026-11-24) |
|---|---|
| A framework, not an automation library; no egui adapter. Its headless tooling drives `rune::Runtime`. | README: *"A native GPU-rendered Rust UI framework … with its own renderer, text, layout and springs."* <https://github.com/anomalyco/rune/blob/dev/README.md> |
| Its `[patch.crates-io]` patches `wgpu-hal`, `naga`, `winit`, `sctk-adwaita`, `parley_engine`, `accesskit_windows`, `accesskit_macos`, `accesskit_atspi_common` — a one-for-one collision with this repo's `winit` patch. Cargo allows one patch per package name. | Rune root `Cargo.toml`; this repo patches `winit = storytold/winit@a517b3846…` in `[patch.crates-io]`. |
| No live inspector socket; headless CLI flags only, so no agent surface without adopting the framework. | *"There is still no live inspector socket; the headless CLI flags (`--tree`, `--snap`, `--trace`, `--script`) stand in."* <https://github.com/anomalyco/rune/blob/dev/crates/rune/README.md> |
| Immature: `version = "0.0.1"`, no releases or tags, repo created 2026-10-22, ~92 commits/day, ~95% of commits from two accounts, external PRs not accepted (collaborators only), one macOS test workflow. Pin these numbers to the retrieval date — the repo moves daily. | <https://github.com/anomalyco/rune> |
| Licence: Rune is MIT; this repo is GPL-3.0-only. No Rune code or design is reused. | — |

The craft `automation` channel supplies the live surface Rune lacks, so no part of Rune is needed.

## 3 · Adopted stack

All components are already pinned; no new git source is added.

| Need | Component | Pin |
|---|---|---|
| In-process semantic driving | `craft::ui::testing::{Fixture, Frames}` — `craft` feature `ui-testing` | craft-libs `243d68e12d05e89122db6d154924bf1992e10d38` |
| Offscreen PNG snapshots | `egui_kittest` 0.37 (`snapshot` + `wgpu`), resolved through the existing `[patch.crates-io]` entry in `Cargo.toml` | egui fork `7e1c30d142bc7879d043e74b5ceb3ecfe7f51329` |
| Live agent/CLI control | `craft` feature `automation` → `craft_automation`: `ControlChannel`, `synthetic`, `dialogs`, `Captures`, `Reply::access_tree`, `server::listen`, `ControlClient` | craft-libs `243d68e12…`; keep `craft`'s **`ui`** feature on — the channel's egui half is gated `craft-automation/ui`, which the façade's `ui` feature supplies (`craft/Cargo.toml`) |
| Agent discovery + MCP | deferred to EPIC-14 (`control-agents` / `automation-agents`, `craft-control::agents::Live`) | — |
| Cross-language contract | `tools/parity/schema.json` + a test-support observation module (no shared Rust crate) | — |

**Not built:**

- **No `rivercrossing-automation` crate** — one consumer; the Python contract is JSON.
- **No harness binary** — `cargo nextest -P ui` is the runner; the Tier B smoke drives the real
  `rivercrossing` binary.
- **No `automation` cargo feature** — features are not profile-scoped, so craft's workspace
  `automation` feature is enabled once and `craft-automation` compiles into release too. The
  **whole control acquisition** — flag parsing **and** `ControlOptions::resolve` (which reads the
  `<APP>_CONTROL_*` environment), then `check` and `server_token` — lives in one
  `#[cfg(debug_assertions)]` function with a `#[cfg(not(debug_assertions))]` stub returning `None`,
  mirroring `smoke_quit_after`. Release builds ignore both the flags and the env vars. EPIC-14 adds
  the consented release surface through `AgentSettings`.
- **`ui-testing` is dev-only.** It is declared on `rivercrossing-ui-egui`'s `[dev-dependencies]`,
  so resolver 3 keeps craft's test kit out of release builds. `egui_kittest` is likewise a
  dev-dependency with `["snapshot","wgpu"]`.
- **Dependency growth is accepted as unavoidable**: `craft/automation` pulls `craft-automation`,
  `keymap`, `base64`, `png`, `hmac`, `sha2`, `getrandom`; `egui_kittest` adds `kittest 0.5`,
  `accesskit_consumer`, `dify`, `open`, `image` (dev-dependency only). `Cargo.lock` is regenerated
  and committed with the change; every gate step uses `--locked`.

## 4 · Selector contract

```rust
enum Selector {
    Action { id: String },                       // frozen §15b id, e.g. mi_finish_ride
    RoleAndLabel { role: String, label: String, window: Option<String> },
}
```

- **Action id first.** Every Action carries the frozen snake_case id (architecture §5); it is the
  primary selector for commands, buttons and menu rows.
- **AccessKit role + name second**, with an optional `window` scope. `spec.md` §15b makes names
  unique only within a top-level window, and `plate_input`, `reason_input`, `continue_btn` and
  `message_lbl` legitimately repeat, so a repeated label is resolved by naming its window. The role
  is a string matching the AccessKit `format!("{:?}", role)` spelling, case-insensitively.
- **No `NodeId`** — per-run, not reproducible. **No bare label** — role-free lookup is a test-side
  convenience, not a contract. Bounds/point selection is added only when the first command-less
  canvas target exists (R-87's last resort).
- **Multiple matches without a `window` are an error**, never an arbitrary pick.
- **The live channel resolves the selector app-side.** Craft's synthetic primitives take
  coordinates or text, not selectors, so the app walks `WindowHost::access_tree(ROOT)` to the
  matching node's bounds and injects the primitive. `ui.accessTree` is ROOT-only at this rev, so a
  `window` scope is a Tier A/schema field only until EPIC-12.
- The same selector type is used by Tier A, the scenario JSON, the live channel's params and the
  Tier C Python driver, so an id or accessible name is the single stable handle across all tiers.

## 5 · Test tiers

| Tier | Surface | Runner | Runs on | Needs |
|---|---|---|---|---|
| **A — semantic** | `craft::ui::testing::Fixture` in `src/ui/**/tests.rs` | `cargo nextest run -P unit --locked` | forge box (gate) | no display, no GPU |
| **A-snapshot** | `egui_kittest` `Harness::builder().with_context_setup(...).build_ui(...)` in `tests/ui/main.rs` | `cargo nextest run -P ui --locked` | forge box (golden host) | Linux + lavapipe |
| **B — real window** | existing `open_and_quit_smoke` + one `control_channel_smoke` | run by hand, per-EPIC gate | forge box under `xvfb-run` | Xvfb; no WM |
| **C — Python parity** | shared JSON scenarios: in-process pytest module in the oracle repo + the Rust scenario runner | at EPIC-12 exit | **Mac** (the forge box cannot launch the wx app) | Python 3.14 / wxPython 4.3.1 oracle repo |

Tier A uses `Fixture::new`, which takes a move-only `FnMut(&mut Ui) + 'static`; the app is therefore
shared as `Rc<RefCell<RiverCrossingApp>>` (`Fixture::new(move |ui| app.borrow_mut().ui(ui))`), with
`app.borrow_mut().logic(ctx)` where the logic pass is under test. `Fixture::new` installs
`install_spec()` (stories on); only `Frames::over` can disable them. R-89's
`craft::ui::testing::access::violations` must be called explicitly after every settled frame —
`Fixture::settle` does not run it. A text field needs an explicit `.accessible_name(...)`:
`hint_text` becomes the AccessKit placeholder, not the name.

Tier A-snapshot uses `Harness::builder().with_context_setup(...).build_ui(...)`;
`embed::before_input` runs **between** passes (it debug-asserts outside one), and `embed::logic` +
`app.ui(ui)` run inside the pass. `Harness::idle` does not exist in this rev.

Tier B is exactly two smokes — open/quit and the control channel — and nothing else. Tier C is on
demand and is **not** part of the Rust gate.

## 6 · Snapshot policy

- **One golden host**: Linux + lavapipe on the forge box. Goldens are regenerated only there, with
  `UPDATE_SNAPSHOTS=force cargo nextest run -P ui --locked`.
- **Per-OS thresholds**, never a second golden set and never a skip: `kittest.toml` carries
  `output_path = "tests/snapshots"` (package-relative), `threshold = 0.6`, `[linux] threshold = 0.6`
  and `[mac] threshold = 2.0`. `kittest.toml` is found by walking up from the **CWD**, which
  nextest sets to the package root — so goldens land in
  `crates/rivercrossing-ui-egui/tests/snapshots`. The key is `[mac]`, not `[macos]`.
- **Fonts are bundled**: install the app fonts (`craft::ui::fonts::UiFonts`) in the harness.
  `allow_missing_glyphs` stays off — kittest panics on a glyph no font can draw, which is the
  intended failure.
- **Failures are artifacts**: `.diff.png`/`.new.png` (`.old.png` on update) are written beside the
  golden and ignored by git; only the `.png` goldens are committed.
- Both the `snapshot` and `wgpu` features are required: `wgpu` renders, `snapshot` compares and
  reads `kittest.toml`.
- `KITTEST_RECORD` is out of scope — it needs the `video` feature and `ffmpeg`.
- No `#[ignore]`, no platform skips: platform variance is a threshold, not a skip (R-90).

## 7 · Live control security model

The channel is a local test/debug surface, not a network service. It exists in debug builds only
until EPIC-14 adds the consented release surface.

| Control | Mechanism |
|---|---|
| Acquisition | Whole acquisition (flag parsing **and** env resolution) in one `#[cfg(debug_assertions)]` function; `#[cfg(not(debug_assertions))]` returns `None`. Release ignores `--control*` and `<APP>_CONTROL_*`. The acquisition is `ControlOptions::resolve` → `check` → `server_token`. |
| Bind | `server::listen` binds `127.0.0.1` only; port `0` lets the OS pick. |
| Token | 32 random bytes as 64 hex chars (`TOKEN_HEX_LEN`); a caller-supplied file is read, a generated token is printed to stderr. Challenge–response handshake (`auth` / `auth.challenge`) with a 10 s auth deadline. |
| Request bounds | 1 MiB per request (`MAX_REQUEST_BYTES`), 16 connections (`MAX_CONNECTIONS`), 30 s I/O timeout. |
| Write root | `<current_dir>/target/control`, canonicalised; every app-owned write rejects absolute paths, `..` and symlink escapes; PNGs land under it. |
| Reply deadline | `REPLY_DEADLINE` is 60 s. Replies bound to injected input settle after the pass that consumed it (`ControlChannel::input_processed`); `ControlChannel::poll_input(&WindowHost)` must run once per pass after dispatching synthetic input or `ui.click`/`ui.key`/`ui.type` stall to the deadline. |
| Lifetime | `Listening`'s `Drop` stops the server; the app owns `Option<Control>` so the guard lives exactly as long as the window. `ControlChannel` and `Captures` have no `Debug`, so `RiverCrossingApp` hand-writes `Debug`. |
| AccessKit | `ctx.enable_accesskit()` is called once in `run`'s `make(&Context)` closure; craft has no install hook. Without the call the live tree is empty. |
| Diagnostics | The listening address is printed with `writeln!(std::io::stderr(), …)` (or a reasoned `#[expect(clippy::print_stderr)]`) — the workspace denies the print macros. |

Methods: craft's `synthetic::input_method` provides the coordinate/text primitives
`ui.click`/`ui.move`/`ui.wheel`/`ui.key`/`ui.release`/`ui.type`/`ui.ime`/`ui.capsLock`, and
`dialogs::dialog_method` handles `ui.dialog.set`/`confirm`/`cancel`. The app owns `ui.accessTree`
(craft's flat ROOT tree), `ui.command` (any frozen action id, so `mi_new_ride` opens the setup
dialog and the automation-only `app_quit` closes the app; a native confirmation answers
`ui.dialog.confirm`/`cancel` when no `DialogStack` dialog is open), `ui.resize`,
`ui.screenshot`, and the selector-aware `ui.click`/`ui.type` that resolve a `Selector` to bounds
before injecting the primitive.

## 8 · Scenario model and parity schema

```rust
Scenario { version, id, size, steps }
Step::{ Command, Click, Type, Key, Snapshot, Expect }
Observation { app, version, viewport, tree: WireTree, screenshots }
WireTree { nodes: Vec<Node>, focus: Option<u64>, root: Option<u64> }
Node { id, role: String, name: Option<String>, value: Option<String>, children: Vec<u64> }
```

- `Command` takes an Action id and optional params; `Click`/`Type`/`Key` take a `Selector`.
- `Step::Expect` is the only assertion mechanism at EPIC-10 — there is no separate compare
  subcommand.
- The JSON is versioned (`"version": 1`). `tools/parity/schema.json` documents the same shape for
  the Python driver. A unit test `include_str!`s the schema and asserts its
  `required`/`enum`/`properties` match the serde derive's field and variant names — a hand-written
  structural check, no `jsonschema`/`schemars` dependency.
- The model lives in `src/ui/automation/scenario.rs` as a public app module so both the unit tests
  and the `ui` target can use it. It is automation API, not a new crate.
- `Observation.tree` mirrors craft's flat `ui.accessTree` reply (`nodes`, `focus`, `root`); `Node`
  is flat, and `textSelection` is a documented subset omission (serde ignores it).
- Fields dropped until a caller exists — `title`, `pixels_per_point`, per-scenario `os`, `Resize`,
  `Set`, `Wait`, `Observation.model`, `Node.bounds`/`selected` — return with EPIC-12's first
  screen.
- Tier C writes `Observation` JSON and PNGs to `target/gate/parity/<scenario>/`; a missing Python
  prerequisite is recorded in the committed `docs/parity/SKIP.md` with its reason, never as a pass.

## 9 · EPIC-10 / EPIC-12 scope split

| Requirement | EPIC-10 | EPIC-12 |
|---|---|---|
| **R-87** — every surface drivable headlessly by a stable selector (Action id first, AccessKit role+name second); pixel coordinates a last resort for command-less targets | mechanism + shell/probe targets | every screen and dialog target |
| **R-88** — opt-in, token-authenticated, loopback-only JSON control channel; writes confined to the automation root | channel, method table, security model complete; the three `ui.dialog.*` methods return a typed "no hosted dialog" until EPIC-12 | `ui.inspect`/`ui.set` and real dialogs |
| **R-89** — every input widget carries an accessible name; the accessibility check runs on every settled frame and fails the suite | check wired into the harness | asserted per screen/dialog |
| **R-90** — offscreen PNG goldens for every screen/dialog state from one golden host; per-OS thresholds, never skips | shell golden + policy | one golden per screen state |
| **R-91** — one shared scenario schema drives Python and Rust; parity compares structured trees and screenshots | schema round-trip + Tier C smoke (or recorded `SKIP`) | parity run: 0 structured mismatches |
| **R-92** — semantic UI tests in the unit sweep plus a `ui` profile; exactly two real-window smokes (amends R-74) | gate, profile, both smokes | — |

R-87, R-90 and R-91 are **partially satisfied at EPIC-10 and completed at EPIC-12**; R-88's dialog
clause is also partial here; R-89 and R-92 are satisfied here. EPIC-10 does not ship a partial
frozen-id manifest: the §15/§15b catalog arrives with EPIC-12's screens.

**EPIC-12 inherits**, per screen/dialog: the `ctx.enable_accesskit()` call, an accessible name on
every input, every §15/§15b id in the `ActionRegistry` (with `enabled()`, shortcut and target), one
semantic test and one snapshot golden per screen state, the frozen-id manifest and its drift check,
real `ui.inspect`/`ui.set` fields, and the Tier C parity report.

## 10 · Citations

- craft-libs `243d68e12d05e89122db6d154924bf1992e10d38`:
  `crates/automation/src/{control,synthetic,screenshot,dialogs,args,server,security,client,protocol}.rs`,
  `crates/ui-contract/src/testing/{fixture,frames,snapshot,access}.rs`,
  `crates/ui-contract/src/runtime/windows/host.rs`,
  `crates/ui-shell/src/shell/{app,embed}.rs`, `crates/craft/Cargo.toml`.
- egui fork `7e1c30d142bc7879d043e74b5ceb3ecfe7f51329`:
  `crates/egui_kittest/src/{lib,builder,config,snapshot,accessibility,wgpu,node}.rs`,
  `crates/egui/src/{response.rs,data/output.rs}`.
- Rune: <https://github.com/anomalyco/rune> (README, `ARCHITECTURE.md`, `crates/rune/README.md`,
  root `Cargo.toml`), retrieved 2026-11-24.
- Repo: `Cargo.toml` (workspace deps and `[patch.crates-io]`),
  `crates/rivercrossing-ui-egui/src/ui/app.rs`, `apps/rivercrossing/src/main.rs`,
  `apps/rivercrossing/tests/functional/main.rs`, `xtask/src/layers.rs`, `.config/nextest.toml`,
  `scripts/validate-rust.sh`, `design/docs-md/spec.md` §12 (functional harness) and §15b (naming
  uniqueness), `requirements.md` §10 (R-87…R-92).

Companions: [architecture](architecture.md) · [test plan](test-plan.md) · [engineering spec](spec.md)
· [requirements](requirements.md) · [project plan](project-plan.md).
