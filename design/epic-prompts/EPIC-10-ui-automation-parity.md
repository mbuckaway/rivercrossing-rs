# EPIC-10 — UI automation & Python-parity harness

Load the `.rs` work into a `tdd-rust-writer` agent (TDD, RS-14); the docs and the parity scaffold stay
in the main session, and `.sh`/`.gitignore` work goes to `shell-code-writer`. Run after EPIC-9, before
EPIC-12. This EPIC has **no single crate owner** — it is the one deliberate exception to "one crate per
EPIC": the model lives in the UI crate's `tests/ui/` support module and the live channel is glue in the
existing app and UI crate.

## Goal

Deliver the automation **mechanism** before the UI: in-process semantic tests on craft's `Fixture`,
deterministic `egui_kittest` snapshots, the live token-authenticated control channel, the versioned
scenario schema, and the Python-parity scaffold — so every EPIC-12 screen inherits an executable test
contract, an agent-drivable surface, and a parity oracle instead of being built blind.

## Context

See `automation.md` for the full decision and evidence (Rune evaluated and rejected; egui-native stack
adopted) and `test-plan.md` for the flow → tier → assertion matrix. Constraints:

- **No new crate, no new binary, no new cargo feature.** The scenario/observation model is a
  test-support module in `crates/rivercrossing-ui-egui/tests/ui/scenario.rs`; the runner is
  `cargo nextest -P ui`; the live channel is `craft`'s workspace `automation` feature driving the
  existing `apps/rivercrossing` binary. The cross-language contract is JSON, not Rust types.
- **Debug-only acquisition.** The whole `ControlOptions` acquisition (`resolve` → `check` →
  `server_token`, reading the hand-rolled `--control` flags **and** the `<APP>_CONTROL_*`
  environment) sits in one `#[cfg(debug_assertions)]` function with a
  `#[cfg(not(debug_assertions))]` stub returning `None`; release builds ignore flags and env vars.
  The consented release surface belongs to EPIC-14.
- **The headless proof is the gate.** The Tier B control smoke must pass under `xvfb-run` on the
  forge box, with its log at `target/gate/control-smoke.log`; a compile check does not satisfy it.
- **Selector resolution is app-owned.** Craft's synthetic primitives take coordinates or text, not
  selectors, so the app resolves `Selector::{Action, RoleAndLabel}` against the AccessKit tree and
  injects the primitive. `ui.accessTree` is ROOT-only at this rev.
- **Accessible names are explicit.** A text field's `hint_text` is a placeholder, not its
  AccessKit name; the probe field must call `.accessible_name("Probe Input")`.
- **Scenario model home:** `src/ui/automation/scenario.rs`; `Observation.tree` mirrors craft's flat
  `accessTree` reply.
- Tiers: **A** semantic (`Fixture`, `-P unit`), **A-snapshot** (`egui_kittest`, `-P ui`), **B** real
  window (control smoke), **C** Python parity (in-process pytest driver in the oracle repo, Mac).
- R-87…R-92 are the Rust-added acceptance ids. R-87/R-90/R-91 are **partially satisfied here,
  completed at EPIC-12**; the rest are fully satisfied at this EPIC's exit.
- The full §15b catalog and the screens are **EPIC-12**. EPIC-10 ships only debug probe actions
  (`probe_echo`, `probe_quit`) and one shell golden.

## Refs

- `design/docs-md/automation.md` (decision, stack, selector, security, snapshot policy, parity schema)
  · `design/docs-md/test-plan.md` (tier matrix, golden policy, artifacts)
- `architecture.md` §12 · `spec.md` §16 · `CODE_STRUCTURE.md` (`tests/ui` + `src/ui/automation` seam)
- `requirements.md` R-87…R-92
- `crates/rivercrossing-ui-egui/tests/ui/` and `src/ui/**/tests.rs` (test templates),
  `apps/rivercrossing/tests/functional/main.rs` (Tier B), `tools/parity/` (Tier C)

## Tests-first (Red)

- **Method table** — craft's coordinate/text primitives (`ui.click`/`move`/`wheel`/`key`/`release`/
  `type`/`ime`/`capsLock`) plus the app-owned `ui.accessTree`, `ui.command` (`probe_echo`,
  `probe_quit`), `ui.screenshot`, `ui.resize` and the selector-aware `ui.click`/`ui.type`: an
  unknown method, bad params and a missing/duplicate selector each return a typed error, never a
  panic. A bad or missing token is refused by the handshake before any method runs.
- **Selector** — an Action id resolves first, role+label second; a `window` scope disambiguates two
  probe nodes sharing one label; an unscoped multiple match is an error. The live resolver walks
  the AccessKit tree and reads the node's bounds; `ui.accessTree` is ROOT-only at this rev.
- **Scenario round-trip** — `Scenario`/`Step`/`Selector`/`Observation`/`WireTree`/`Node` serialise
  and deserialise to `"version": 1` JSON; a 4-step scenario drives a probe UI closure (button, text
  field, Tab, `Expect`) and asserts the tree; `include_str!` of `tools/parity/schema.json` asserts
  its `required`/`enum`/`properties` match the serde derive (hand-written check, no schema crate).
  The model lives in `src/ui/automation/scenario.rs`; `Observation.tree` mirrors craft's flat
  reply.
- **Path escape** — `ui.screenshot` writes only inside the canonicalised `<cwd>/target/control` root
  and rejects absolute paths, `..` and symlink escapes.
- **Accessibility** — every input widget exposes an accessible name. A text field needs an explicit
  `.accessible_name("Probe Input")`: `hint_text` is only the placeholder.
- **Snapshot** — one `egui_kittest` golden of the shell within the `kittest.toml` threshold.
- **Control smoke (Tier B)** — spawn `rivercrossing --control 0 --control-token <64hex>`, drain
  stderr to the listening line, connect, then assert `ui.accessTree` root, `probe_echo`,
  selector `ui.click`/`ui.type`, `ui.screenshot` PNG, a rejected `../` path, `ui.resize`, and
  `probe_quit` exit 0. Also assert a release build ignores `--control` and the env vars.

## Implement (Green → Refactor)

1. **T1 — Decision + contract docs** (main session): `automation.md`, `test-plan.md`, architecture
   §12, spec §16, R-87…R-92, the `AGENTS.md` bounded test exception, and the EPIC-10…14 renumber.
2. **T2 — Harness skeleton**: add `egui_kittest` to the workspace deps (it is only a patch entry
   today) and craft's `ui-testing` on the UI crate's **dev-dependencies**; `tests/ui/main.rs`
   (harness smoke, then the shell golden); `kittest.toml`; `[profile.ui]` + `[[test]] name = "ui"`
   landing together; regenerate and commit `Cargo.lock`.
3. **T3 — Live control channel**: `src/ui/automation/` — `ControlConfig { options, token }`,
   `ControlChannel` in `logic`, `captures.note_windows`/`drain`/`finish_captures`, `note_pass`/
   `issue`/`finish_captures` in `ui`, `poll_input`/`poll_access_trees` each pass, `settle_ms` from
   the animation time, `DocumentId = None`; the method table above; `run(quit_after, control)`;
   `ctx.enable_accesskit()` in `make`; canonicalised `<cwd>/target/control`; hand-written `Debug`.
4. **T4 — Scenario model + expectation**: `src/ui/automation/scenario.rs` (public automation API,
   serde-only); `tools/parity/schema.json` created here; JSON round-trip; selector resolution;
   `Expect` evaluator; the schema structural test.
5. **T5 — Golden + gate**: `validate-rust.sh` `ui` stage and release `cargo check`; `kittest.toml`
   thresholds; `.gitignore`; `xtask/src/layers.rs` `FORBIDDEN` 5 entries + `layers/tests.rs`
   violation fixture; `COVERAGE_IGNORE` for the channel glue.
6. **T6 — Tier B smoke**: `control_channel_smoke` beside `open_and_quit_smoke`; the canonical
   `xvfb-run` command; artifacts to `target/gate/control-smoke.log` and `target/control/`. Do
   **not** set `RIVERCROSSING_SMOKE_QUIT_AFTER_MS` — `probe_quit` is the deterministic exit.
7. **T7 — Parity scaffold** (main session): `tools/parity/` (README, one Python smoke scenario,
   in-process driver stub); `docs/parity/SKIP.md`; record the oracle SHA/version.
8. **T8 — Verification + handoff** (main session): full gate, Tier B, Tier C smoke or SKIP, runbook
   update, the EPIC-12 handoff checklist.

## Done when

- `cargo build --locked`, `cargo check -p rivercrossing --release --locked`,
  `cargo nextest run -P unit --locked`, `WGPU_BACKEND=vulkan cargo nextest run -P ui -p
  rivercrossing-ui-egui --locked` and `./scripts/validate-rust.sh` are all green on the forge box.
- **The headless proof passes**: the canonical control smoke under `xvfb-run` records
  `target/gate/control-smoke.log` with every assertion, and `probe_quit` exits 0.
- The release binary, run with `--control 0` under Xvfb, survives and never prints
  `control server listening`.
- The live app answers `ui.accessTree`, `ui.command probe_echo`, selector `ui.click`/`ui.type`,
  `ui.screenshot`, `ui.resize`; a `../` path is rejected.
- At least one shell golden exists and `-P ui` selects only the `ui` target.
- The parity item is the **schema round-trip plus either a Tier C smoke run or a committed
  `docs/parity/SKIP.md`** with the exact reason — "0 structured mismatches" belongs to EPIC-12
  exit, when screens exist.
- The EPIC-12 handoff checklist is written and `docs/ui-porting-runbook.md` is updated.
