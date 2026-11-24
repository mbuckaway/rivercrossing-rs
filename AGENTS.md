# RiverCrossing-rs — Agent Rules

Guidance for AI coding agents and human contributors working in the **rivercrossing-rs** repository.
RiverCrossing-rs is the **Rust / egui** port of the RiverCrossing poker-run ride-timing desktop app:
it reproduces the Python app's screens and functionality as a layered Cargo workspace built on
**egui + eframe + wgpu** through the ArtCraft **`craft-libs`** crates. Per-language detail lives in the
`CODINGSTANDARDS-*.md` files at the repo root; the product contract lives in `design/`. `CLAUDE.md` is
a symlink to this file.

---
TEST RULES

UNIT TESTS ARE THE DEFAULT. ALL CODE MUST BE WRITTEN USING TDD AND THE `tdd-rust-writer` AGENT.

The EPIC-10 UI-automation harness (R-87…R-92; R-92 amends R-74) is the one bounded exception.
Exactly these test categories are authorized:

- **Tier A semantic** — `craft::ui::testing::Fixture` tests in `src/**/tests.rs`, run by
  `cargo nextest run -P unit --locked`.
- **Tier A-snapshot** — `egui_kittest` snapshot tests in
  `crates/rivercrossing-ui-egui/tests/ui/main.rs`, run by `cargo nextest run -P ui --locked`.
- **Tier B real-window smokes** — exactly two: `open_and_quit_smoke` and the control-channel smoke
  (`--control 0`, `ui.accessTree`, `ui.command probe_echo`, `ui.screenshot`,
  `ui.command probe_quit`), run by hand under Xvfb, outside the `unit`/`ui` sweep.

NO OTHER FUNCTIONAL, BEHAVIOURAL, ACCEPTANCE, SIMULATOR OR OTHER TESTS ARE TO BE WRITTEN WITHOUT
EXPRESSED PERMISSION. Tier C Python parity runs on demand at EPIC-12 exit, not in the gate. No
`#[ignore]`, no skips. Platform variance is absorbed by per-OS snapshot thresholds, never by
disabling a test.
---

## `design/` is the contract

`design/` is the build contract — the numbered requirements (R-ids), the engineering spec, the egui
command/widget-id registry, the Rust project plan, and agent-ready EPIC prompts. A developer or agent
who was not part of the design conversation should be able to build the Rust port from it alone.

| Path | Role |
|---|---|
| `design/docs-md/requirements.md` | Numbered **R-ids** — the acceptance authority (R-87…R-92 are the Rust automation requirements) |
| `design/docs-md/spec.md` | Engineering spec — §1–§15b are the Python oracle; §12/§14 describe the Python test/CI stages and point at §16; §16 is the Rust automation mechanism |
| `design/docs-md/xrc-windows.md` | The Python window inventory and the §15b frozen control-name registry — **implementation truth for UI** |
| `design/docs-md/architecture.md` | The Rust architecture (layered crates, Action registry, persistence, exports, §12 automation) |
| `design/docs-md/automation.md` | The Rust automation decision (Rune rejected), adopted stack, selector contract, tiers, snapshot policy, parity schema |
| `design/docs-md/test-plan.md` | The Rust test matrix — flow → tier → mechanism → assertion, golden policy, tier owners/artifacts |
| `design/docs-md/project-plan.md` · `task-briefs.md` | Rust EPIC plan and per-task briefs |
| `design/epic-prompts/` | Agent-ready EPIC prompts EPIC-1…14 (load each into a `tdd-rust-writer` agent; EPIC-10 has the documented owner split) |
| `design/screenshots/` | Python window screenshots — the visual source for the Rust UI |
| `design/exports/*.html` | Golden results pages; their `race-data` JSON is the export oracle |

`design/docs-md/` is canonical for agents. `design/docs-html/` (if present) is a stale mirror.

---

## Answering & research discipline

- **Never invent** APIs, flags, file paths, crate names, control names or behaviour. Use only what is
  in the code, `design/`, the standards, or well-documented standard behaviour.
- When a request is uncertain, underspecified or ambiguous, **ask** — with researched options and a
  recommendation — rather than guessing.
- **If a design document is silent or two documents contradict each other, stop and ask.**

## Plans

Non-trivial work starts with a plan. State a confidence level backed by evidence; aim for ≥ 90% before
implementing.

---

## Coding standards — read pattern (MANDATORY)

Before writing or editing code, read the relevant standard(s) at the repo root and apply them:

```
IF editing .rs                        → read ./CODINGSTANDARDS-RUST.md
IF editing .sh                        → read ./CODINGSTANDARDS-SHELL.md
IF writing/editing any code           → also read ./CODINGSTANDARDS-SIMPLECODE.md
IF designing/editing user-facing UI   → read ./CODINGSTANDARDS-UX.md
                                         and ./CODINGSTANDARDS-UX-DESKTOP.md
APPLY the standard. If code would violate it, STOP and ASK, naming the rule.
```

Source files carry a single SPDX line and no other licence header:

```rust
// SPDX-License-Identifier: GPL-3.0-only
```

---

## TDD & testing

- **Test-first, always.** Production code is written by the **`tdd-rust-writer`** agent using strict
  Red-Green-Refactor: write the named failing tests, watch them fail, make them pass, refactor. A
  change whose first commit is not tests is rejected.
- **Coverage ≥ 90% line AND 90% branch** on the domain crates (RS-13), enforced by the gate.
- **Unit tests only, by default.** Tests live in `src/<module>/tests.rs` (`#[cfg(test)] mod tests;`).
  The EPIC-10 automation harness (R-87…R-92) adds the three authorized categories in the TEST RULES
  block above — Tier A semantic (still `src/**/tests.rs`, `-P unit`), Tier A-snapshot
  (`tests/ui/main.rs`, `-P ui`) and the two Tier B real-window smokes (manual). Tier C parity is an
  on-demand EPIC-12-exit run, not a gate stage. No other functional, behavioural, acceptance,
  simulator or property tests without expressed permission.
- Use `rstest` (fixtures/cases), `pretty_assertions`, and `mockall` at I/O boundaries (trait seam).
  No `#[ignore]`, no skips, no `assert true`-style vacuous tests, `doctest = false`.
- **Run `tdd-rust-writer` agents sequentially** — concurrent whole-crate test compiles collide.

## Architecture rules

- **Layered workspace.** Domain crates (`rivercrossing-cards`, `-hands`, `-standings`, `-ride`,
  `-roster`, `-rider-issues`, `-store`, `-csvio`, `-htmlexport`, `-pdfexport`, `-wordpress`) are
  UI-agnostic; `rivercrossing-ui-egui` is the only crate allowed to depend on egui/eframe/winit.
- **Nothing below `ui-egui` depends on egui** — enforced by `cargo xtask layers`.
- **`egui_kittest` is an egui-family dependency** — forbidden below `rivercrossing-ui-egui` (in the
  `cargo xtask layers` `FORBIDDEN` list) and allowed in `apps/`. The automation test-support module
  lives in `crates/rivercrossing-ui-egui/tests/ui/`, not in a new crate.
- **Command/Action registry.** The §15 menu map + §15b frozen-name registry become one `Action`
  registry (stable id, label, shortcut, `enabled()`, target). Menu rows and on-screen buttons dispatch
  the same id; an ids-drift check fails CI on drift.
- **craft-libs pin.** One shared git `rev`; commit `Cargo.lock`; build `--locked`; move the rev only in
  its own PR. The pinned build is craft-libs `243d68e12d05e89122db6d154924bf1992e10d38` (`version =
  "=0.1.0"`), egui/eframe `0.37.0` at `7e1c30d142bc7879d043e74b5ceb3ecfe7f51329`, winit `0.30.13` at
  `a517b3846ee8457d84ee4975110bfda6797c44dc`, wgpu `30.0`. The egui/winit forks are repeated in this
  workspace's `[patch.crates-io]` because they do not propagate from craft-libs.
- **Where the build runs.** This repo has no CI workflow: the fleet runners' golden AMI carries no Rust
  toolchain, and the forge host runner executes as user `git`. Builds and gates run on the forge box
  (`dev`) and on the Mac over SSH. If this repo is later mirrored to the Forge, the job must target the
  host runner (`runs-on: [linux, ARM64]`), never a `box`/`fleet` label, and needs a `git`-owned
  toolchain.
- **`forbid(unsafe_code)`** per domain-crate root; no `unwrap`/`expect`/`panic` in production.
- **Agent surface (EPIC-14).** The CLI and the MCP server both dispatch the same `Action` registry; the
  MCP server uses the shared **`craft-mcp-server`** crate (craft-libs), and the CLI is a hand-rolled
  parser (no `clap`).
- Keep functions short and single-purpose.

## Build & test

```bash
cargo build --locked
cargo nextest run -P unit --locked
cargo nextest run -P ui --locked                  # Tier A-snapshot goldens (egui_kittest)
cargo check -p rivercrossing --release --locked   # proves the debug-gated control path is inert
./scripts/validate-rust.sh     # the per-PR gate (fmt, clippy, release check, doc, nextest list,
                               # msrv/hack, deny, machete, layers, ui, llvm-cov)
```

---

## Source control

- **Never commit to `master`** — use `topic/*` branches. Release version bumps go on a
  `release/v<X.Y.Z>` branch merged by PR.
- Conventional Commits, scope = crate. Test-first shows in history: `test(hands): <id> red`, then
  `feat(hands): <id> green`.
- **No AI advertising** — never add "Generated with …", AI co-author trailers, or similar text to
  commits, code, comments, PR bodies or documentation.

## Internal tools

Prefer the harness's dedicated file/search tools (Read, Edit, Write, Grep, Glob) and available MCP
tools over ad-hoc `cat`/`sed`/`grep`/`find` shell commands for file operations and searches.
