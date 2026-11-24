# CODINGSTANDARDS-RUST.md — rivercrossing-rs

The canonical Rust standard is `${HOME}/.agents/standards/CODINGSTANDARDS-RUST.md`
(RS-1…RS-24). This file is a **project tightening**, not a replacement. Where the two differ,
the global standard wins unless this file explicitly tightens a rule.

## rivercrossing-rs specifics

- **License / headers.** GPL-3.0-only. Every `.rs` carries a single
  `// SPDX-License-Identifier: GPL-3.0-only` line and no other copyright/licence text
  (CODINGSTANDARDS-COPYRIGHT.md, SPDX-only default).
- **Workspace layout.** `crates/rivercrossing-*` (domain, UI-agnostic) →
  `crates/rivercrossing-ui-egui` (L6, the only crate allowed to depend on egui/eframe/winit) →
  `apps/rivercrossing` (thin binary). See `CODE_STRUCTURE.md`.
- **Layering.** Nothing below `rivercrossing-ui-egui` may depend on egui/eframe/winit/rfd —
  enforced by the `cargo xtask layers` check.
- **Authorship.** All `.rs` is written by the `tdd-rust-writer` agent, **sequentially** (one
  uncompilable module test file fails every other module's build). Test-first (RS-14).
- **Test categories.** Unit tests are the default. The EPIC-10 automation harness (R-87…R-92) adds,
  and only adds:
  - **Tier A — semantic:** `craft::ui::testing::Fixture` over `Rc<RefCell<RiverCrossingApp>>` in
    `crates/rivercrossing-ui-egui/src/**/tests.rs`; run by `cargo nextest run -P unit --locked`; no
    display, no GPU.
  - **Tier A-snapshot:** `egui_kittest` `Harness::new_ui` snapshot tests in
    `crates/rivercrossing-ui-egui/tests/ui/main.rs`; run by `cargo nextest run -P ui --locked`;
    `tests/ui/scenario.rs` is test-support, not production API.
  - **Tier B — real window:** exactly two manual smokes — `open_and_quit_smoke` and
    `control_channel_smoke` — run under `xvfb-run`, outside the `unit`/`ui` sweep.
  - **Tier C — Python parity:** shared JSON scenarios against the in-process Python oracle, on
    demand at EPIC-12 exit, never in the gate.

  No other functional, behavioural, acceptance, simulator or property suite. No `#[ignore]`, no
  skips, no `assert true`-style vacuous tests, `doctest = false`.
- **Coverage gate.** 90% line AND 90% branch, each alone (RS-13), scoped to the domain crates and
  the UI crate's production code.
  The egui view layer and thin entry points are excluded via `--ignore-filename-regex`
  (mirroring the Python `[tool.coverage.run] omit` list). The Rust paths, as wired in
  `scripts/validate-rust.sh`, are `(^|/)src/main\.rs`, `xtask/src/main\.rs`,
  `crates/rivercrossing-ui-egui/src/ui/app\.rs`, `crates/rivercrossing-ui-egui/src/ui/views/` and
  `crates/rivercrossing-ui-egui/src/ui/ids\.rs`; every other file must hold 90% of both. The
  automation production module (`crates/rivercrossing-ui-egui/src/ui/automation/`) stays **measured**
  — it is not on that list and the aggregate 90/90 holds with it; test-support under `tests/` (e.g.
  `tests/ui/scenario.rs`) is not measured, because the `unit` profile never runs it. The line floor
  is `llvm-cov report --fail-under-lines`; the branch floor is `scripts/branch-coverage.py` reading
  the report's JSON totals, because cargo-llvm-cov has no `--fail-under-branches`.
- **Snapshot (golden) policy.** One golden host — Linux with lavapipe; goldens live under
  `crates/rivercrossing-ui-egui/tests/snapshots` (`kittest.toml` `output_path = "tests/snapshots"`,
  package-relative). Platform variance is a per-OS threshold, never a second golden set or a skip:
  `[linux] threshold = 0.6` on the golden host and `[mac] threshold = 2.0`. Regenerate deliberately,
  on the golden host only, with `UPDATE_SNAPSHOTS=force cargo nextest run -P ui --locked`; review the
  diff and commit the new PNG. Missing glyphs panic by default — install the app fonts
  (`craft::ui::fonts::UiFonts`) and keep `allow_missing_glyphs` off. `*.diff.png`, `*.new.png` and
  `*.old.png` artifacts are git-ignored.
- **craft-libs pin.** One shared git `rev` across all `craft-*` deps; commit `Cargo.lock`; build
  `--locked`. The egui/winit `[patch.crates-io]` forks are repeated in this workspace (they do not
  propagate from craft-libs). Move the rev only in its own PR.
- **Banned crates.** `chrono` (unmaintained), `async-trait` (coverage gate).

The per-PR gate is `scripts/validate-rust.sh`; the exact commands live there. It lists the test
categories (`nextest list`), checks `layers`, runs the `ui` stage and the release `cargo check`, and
ends with the unit + coverage stage.
