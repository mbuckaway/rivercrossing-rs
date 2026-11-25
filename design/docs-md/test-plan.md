# RiverCrossing-rs — UI test plan

Executable matrix for the EPIC-10 automation harness and the EPIC-12 screen suite. Mechanism and
rationale: [automation](automation.md). Architecture: [architecture](architecture.md) §12.
Requirement ids: `requirements.md` §10 (R-87…R-92).

## 1 · Rules

- **Tiers A (semantic) and A-snapshot are the gate.** They join `scripts/validate-rust.sh`.
- **Exactly two real-window smokes (Tier B)** are authorized: open/quit and the control channel
  (R-92, amending R-74). Nothing else drives a real window.
- **Tier C (Python parity) is on demand**, at EPIC-12 exit, and is not part of the Rust gate.
- **No `#[ignore]`, no skips, no vacuous assertions.** Platform variance is a snapshot threshold,
  never a skip; a missing Tier C prerequisite is a committed `SKIP` with its reason.
- **No pixel coordinates for command targets.** The selector is the Action id, or AccessKit role +
  name with an optional window scope (R-87). The live channel resolves the selector app-side.

## 2 · Runners and artifacts

| Tier | Command | Artifacts |
|---|---|---|
| A — semantic | `cargo nextest run -P unit --locked` | coverage in `target/gate/coverage.json` |
| A-snapshot | `WGPU_BACKEND=vulkan cargo nextest run -P ui -p rivercrossing-ui-egui --locked` | `crates/rivercrossing-ui-egui/tests/snapshots/*.png` (goldens, committed); `*.diff.png` / `*.new.png` on failure (ignored) |
| B — real window | the two smokes in `apps/rivercrossing/tests/functional/main.rs`, run by hand under `xvfb-run` | `target/gate/control-smoke.log`, `target/control/*.png` |
| C — Python parity | pytest module in the oracle repo + the Rust scenario runner | `target/gate/parity/<scenario>/` (observation JSON + PNGs); `docs/parity/SKIP.md` if blocked |

The gate, in order: fmt · clippy (`-D warnings`) · release `cargo check` · doc · nextest
category/layout · MSRV + feature powerset · deny + machete · `cargo xtask layers` · `ui` · unit +
llvm-cov 90/90 line and branch.

## 3 · Tier A — semantic flows

Run in `crates/rivercrossing-ui-egui/src/ui/**/tests.rs` through `craft::ui::testing::Fixture` over
`Rc<RefCell<RiverCrossingApp>>`. No display, no GPU. The scenario/selector/command/path models are
`src/ui/automation/*` modules, so their tests are ordinary unit tests in the same sweep.

| Flow | Mechanism | Assertion |
|---|---|---|
| Registry dispatch | `Fixture` over the app; `AppState::act` | the action fires; the resulting state is readable |
| Unknown action | direct lookup with an unregistered id | typed error; no panic |
| Bad selector | `Selector::Action` with an unknown id; malformed params | typed error; no panic |
| Selector window scope / duplicates | two candidates with the same label | the scoped lookup resolves; the unscoped lookup errors |
| Accessibility floor | `craft::ui::testing::access::violations` after every settled frame | every input widget has a name (R-89) |
| Scenario round-trip | `Scenario`/`Step`/`Observation`/`Node` serde round-trip | JSON in == JSON out; an invalid step is rejected |
| Schema structure | `include_str!` of `tools/parity/schema.json`, compared with the serde derive | `required`/`enum`/`properties` match field and variant names |
| Expect evaluator | `Step::Expect` over a settled tree | match passes; a mismatch names the node and field |
| Method-table errors (in-process) | unknown method, bad params, path escape | typed error replies; no write outside the automation root |
| Scenario model | a 4-step scenario (command, click, type, `Expect`) over a synthetic tree | the asserted tree matches |

## 4 · Tier A-snapshot — screen goldens

`crates/rivercrossing-ui-egui/tests/ui/main.rs`, `[[test]] name = "ui"`, `cargo nextest -P ui`.
`egui_kittest` `Harness::builder().with_context_setup(...).build_ui(...)` calling craft's public
`embed::install` (`InstallSpec::new`, `NoWindows` — never `install_spec()`); `embed::before_input`
runs before `run`, and `embed::logic` + `app.ui(ui)` inside the pass.

| Flow | Mechanism | Assertion |
|---|---|---|
| Console, empty state | `snapshot("console_empty")` after the frame settles | golden `tests/snapshots/console_empty.png` within threshold (Linux `0.6`; Mac `2.0`) |
| Ride setup open | `ui.command mi_new_ride` before the pass, then `snapshot("setup_open")` | golden `tests/snapshots/setup_open.png` within threshold |
| Fonts | app fonts installed via `craft::ui::fonts::UiFonts`; `allow_missing_glyphs` off | no missing glyph; a tofu glyph fails |
| Golden update | `UPDATE_SNAPSHOTS=force WGPU_BACKEND=vulkan cargo nextest run -P ui -p rivercrossing-ui-egui --locked` on the golden host only | regenerated goldens reviewed and committed |

## 5 · Tier B — real window

Run by hand on the forge box under `xvfb-run -a -s "-screen 0 1400x900x24"`. No window manager, so
OS-level focus and native portals are out of scope. Leave `WGPU_BACKEND` unset for the real-window
run; `WGPU_BACKEND=vulkan` is for offscreen snapshots only.

| Flow | Mechanism | Assertion |
|---|---|---|
| Launch / present / quit | existing `open_and_quit_smoke` (`RIVERCROSSING_SMOKE_QUIT_AFTER_MS=750`) | the process survives, presents, and exits cleanly |
| Live channel — connect | spawn `rivercrossing --control 0 --control-token <64hex>`; a reader thread drains stderr until `control server listening on <addr>`; `ControlClient::connect(addr, token)` | the handshake succeeds |
| Live channel — tree | `ui.accessTree` | a non-empty root with the settled tree |
| Live channel — command | `ui.command mi_new_ride` | `{"ok":true}`; the next tree shows the ride-setup dialog |
| Live channel — input | `ui.click` the console `record_btn` (`role: Button, label: Record`); `ui.type` the console `plate_input` (`role: TextInput, label: Plate`), then the open setup dialog's name field | the click and the closed entry are harmless; the post-input tree shows the typed setup name |
| Live channel — screenshot | `ui.screenshot {path:"smoke.png"}` | PNG bytes; the file is under `target/control/`; a `..` path is rejected |
| Live channel — resize | `ui.resize` | `ok`; the window survives |
| Live channel — quit | `ui.command app_quit` | `{"bye":true}`, the window closes and the process exits 0 |

The control smoke does **not** set `RIVERCROSSING_SMOKE_QUIT_AFTER_MS`; it removes
`RIVERCROSSING_SMOKE_QUIT_AFTER_MS` and `RIVERCROSSING_CONTROL_PORT`/`_TOKEN`/`_TOKEN_FILE` from the
child environment so the flags win. Stderr and PNGs go to `target/gate/control-smoke.log` and
`target/control/`.

Canonical command:

```bash
mkdir -p target/gate
xvfb-run -a -s "-screen 0 1400x900x24" \
  cargo test -p rivercrossing --test functional control_channel_smoke --locked \
  -- --nocapture --test-threads=1 2>&1 | tee target/gate/control-smoke.log
```

## 6 · Tier C — Python parity (EPIC-12 exit)

Runs on the **Mac** (`/Users/mark/src/rivercrossing`, Python 3.14 / wxPython 4.3.1); the forge box
cannot launch the wx app. The Python driver is in-process — `FindWindowByName`, `SetValue` and
posted `wx.CommandEvent` cannot be driven by an external CLI (`spec.md` §12, functional UI tests).

| Flow | Mechanism | Assertion |
|---|---|---|
| Same scenario, both apps | shared JSON scenario through the Python in-process driver and through `cargo nextest -P ui` | structured trees are **exact**; PNGs match within threshold |
| Artifacts | both sides write `Observation` JSON + PNGs | `target/gate/parity/<scenario>/` holds one JSON and one PNG set per scenario |
| Recovery | `git show <sha>:tests/functional/...` archaeology of the deleted suite | if unavailable, fall back to `design/screenshots/` and `design/exports/` goldens and record `SKIP` with the reason |

"0 structured mismatches" belongs to EPIC-12 exit, when the screens exist. At EPIC-10 the parity
item is schema round-trip plus a Tier C smoke run or a committed `docs/parity/SKIP.md` with the exact
reason (repo/path absent, Python/wx version mismatch, or host unreachable — anything else fails).

## 7 · EPIC-12 inherited matrix

Not claimed at EPIC-10 — the shell has no screens. This is the contract EPIC-12 inherits; each row
gets a semantic test and, per state, a snapshot golden.

| Flow | Tier | Mechanism | Assertion |
|---|---|---|---|
| **Action catalog** | A | every §15/§15b id registered in the `ActionRegistry`; `ui.command` dispatch | every menu row, button and shortcut resolves the same id; ids-drift check clean |
| **Enablement** | A | `Action::enabled()` per ride state (DRAFT / RUNNING / FINISHED / REOPENED) | the §15 "Enabled when" column holds for every row |
| **Dialog semantics** | A | dialog methods (`ui.dialog.set`/`confirm`/`cancel`) + focus checks | Esc cancels; Enter activates the default; destructive confirms focus Cancel; focus returns to the opener (R-76) |
| **Plate entry** | A | typed plate + Enter through the console's entry field | crossing recorded; focus stays in the field; the error/flagged cues fire |
| **Editors** | A | rider/team editor add + edit modes | validation refusals surface; lists and record panel agree |
| **CSV** | A | import report and export round-trip through the presenters | conflicts reported; export re-imports to the same roster |
| **Corrections** | A | add-at-time, edit time, void card, undo, reopen → finish again | the corrected standings match the expected result |

## 8 · Ownership and escalation

- **Tier A / A-snapshot**: forge box, inside `scripts/validate-rust.sh`. The forge box is the only
  golden host.
- **Tier B**: forge box, under `xvfb-run`, per-EPIC gate.
- **Tier C**: Mac, at EPIC-12 exit; a blocked run records a committed `SKIP.md` and does not fail
  the Rust gate.
- An unstable golden is regenerated **once** on the golden host with `UPDATE_SNAPSHOTS=force`; a
  structural tree assertion may gate alone until the image is stable. A red golden is never
  silenced with an ignore or a threshold bump on the golden host.

## 9 · Commands

```bash
export PATH="$HOME/.cargo/bin:$PATH"

cargo nextest run -P unit --locked
WGPU_BACKEND=vulkan cargo nextest run -P ui -p rivercrossing-ui-egui --locked
UPDATE_SNAPSHOTS=force WGPU_BACKEND=vulkan \
  cargo nextest run -P ui -p rivercrossing-ui-egui --locked      # golden host only
cargo check -p rivercrossing --release --locked
./scripts/validate-rust.sh

mkdir -p target/gate
xvfb-run -a -s "-screen 0 1400x900x24" \
  cargo test -p rivercrossing --test functional control_channel_smoke --locked \
  -- --nocapture --test-threads=1 2>&1 | tee target/gate/control-smoke.log

xvfb-run -a -s "-screen 0 1400x900x24" \
  env RIVERCROSSING_SMOKE_QUIT_AFTER_MS=750 cargo run -p rivercrossing --locked
```

Companions: [automation](automation.md) · [architecture](architecture.md) · [engineering spec](spec.md)
· [requirements](requirements.md) · [project plan](project-plan.md).
