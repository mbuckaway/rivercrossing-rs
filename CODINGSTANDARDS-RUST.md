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
- **Coverage gate.** 90% line AND 90% branch, each alone (RS-13), scoped to the domain crates.
  The egui view layer and thin entry points are excluded via `--ignore-filename-regex`
  (mirroring the Python `[tool.coverage.run] omit` list). The Rust paths, as wired in
  `scripts/validate-rust.sh`, are `(^|/)src/main\.rs`, `xtask/src/main\.rs`,
  `crates/rivercrossing-ui-egui/src/ui/app\.rs`, `crates/rivercrossing-ui-egui/src/ui/views/` and
  `crates/rivercrossing-ui-egui/src/ui/ids\.rs`; every other file must hold 90% of both. The line
  floor is `llvm-cov report --fail-under-lines`; the branch floor is `scripts/branch-coverage.py`
  reading the report's JSON totals, because cargo-llvm-cov has no `--fail-under-branches`.
- **craft-libs pin.** One shared git `rev` across all `craft-*` deps; commit `Cargo.lock`; build
  `--locked`. The egui/winit `[patch.crates-io]` forks are repeated in this workspace (they do not
  propagate from craft-libs). Move the rev only in its own PR.
- **Banned crates.** `chrono` (unmaintained), `async-trait` (coverage gate).

The per-PR gate is `scripts/validate-rust.sh`; the exact commands live there.
