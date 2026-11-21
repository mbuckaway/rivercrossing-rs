# RiverCrossing-rs — Agent Rules

Guidance for AI coding agents and human contributors working in the **rivercrossing-rs** repository.
RiverCrossing-rs is the **Rust / egui** port of the RiverCrossing poker-run ride-timing desktop app:
it reproduces the Python app's screens and functionality as a layered Cargo workspace built on
**egui + eframe + wgpu** through the ArtCraft **`craft-libs`** crates. Per-language detail lives in the
`CODINGSTANDARDS-*.md` files at the repo root; the product contract lives in `design/`. `CLAUDE.md` is
a symlink to this file.

---
TEST RULES

ONLY UNIT TESTS WILL BE WRITTEN. ALL CODE MUST BE WRITTEN USING TDD AND THE `tdd-rust-writer` AGENT.
NO FUNCTIONAL, BEHAVIOURAL, ACCEPTANCE, SIMULATOR OR OTHER TESTS ARE TO BE WRITTEN WITHOUT EXPRESSED
PERMISSION.
---

## `design/` is the contract

`design/` is the build contract — the numbered requirements (R-ids), the engineering spec, the egui
command/widget-id registry, the Rust project plan, and agent-ready EPIC prompts. A developer or agent
who was not part of the design conversation should be able to build the Rust port from it alone.

| Path | Role |
|---|---|
| `design/docs-md/requirements.md` | Numbered **R-ids** — the acceptance authority |
| `design/docs-md/spec.md` | Engineering spec (§1–§9 behaviour is the Python oracle; §10–§15b rewritten for Rust) |
| `design/docs-md/gui-windows.md` | The egui command/widget-id registry — **implementation truth for UI** |
| `design/docs-md/architecture.md` | The Rust architecture (layered crates, Action registry, persistence, exports) |
| `design/docs-md/project-plan.md` · `task-briefs.md` | Rust EPIC plan and per-task briefs |
| `design/epic-prompts/` | Agent-ready EPIC prompts (load each into a `tdd-rust-writer` agent) |
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
- **Unit tests only.** Tests live in `src/<module>/tests.rs` (`#[cfg(test)] mod tests;`). No functional,
  behavioural, acceptance, simulator or property tests without expressed permission.
- Use `rstest` (fixtures/cases), `pretty_assertions`, and `mockall` at I/O boundaries (trait seam).
  No `#[ignore]`, no skips, no `assert true`-style vacuous tests, `doctest = false`.
- **Run `tdd-rust-writer` agents sequentially** — concurrent whole-crate test compiles collide.

## Architecture rules

- **Layered workspace.** Domain crates (`rivercrossing-cards`, `-hands`, `-standings`, `-ride`,
  `-roster`, `-rider-issues`, `-store`, `-csvio`, `-htmlexport`, `-pdfexport`, `-wordpress`) are
  UI-agnostic; `rivercrossing-ui-egui` is the only crate allowed to depend on egui/eframe/winit.
- **Nothing below `ui-egui` depends on egui** — enforced by `cargo xtask layers`.
- **Command/Action registry.** The §15 menu map + §15b frozen-name registry become one `Action`
  registry (stable id, label, shortcut, `enabled()`, target). Menu rows and on-screen buttons dispatch
  the same id; an ids-drift check fails CI on drift.
- **craft-libs pin.** One shared git `rev`; commit `Cargo.lock`; build `--locked`; move the rev only in
  its own PR.
- **`forbid(unsafe_code)`** per domain-crate root; no `unwrap`/`expect`/`panic` in production.
- **Agent surface (EPIC-13).** The CLI and the MCP server both dispatch the same `Action` registry; the
  MCP server uses the shared **`craft-mcp-server`** crate (craft-libs), and the CLI is a hand-rolled
  parser (no `clap`).
- Keep functions short and single-purpose.

## Build & test

```bash
cargo build --locked
cargo nextest run -P unit --locked
./scripts/validate-rust.sh     # the per-PR gate (fmt, clippy, doc, deny, machete, hack, llvm-cov)
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
