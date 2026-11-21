# EPIC-13 — CLI + MCP (parity complete)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run last — only after 100% Python parity.

## Goal

Expose the command/Action registry via a CLI, a JSON control channel, and an MCP server, so agents
drive every command the GUI can (mirroring ArtCraft's "every button is a command" surface).

## Context

The `Action` registry built in EPIC-11 is the single source of truth. ArtCraft's pattern (verified in
`storytold/wordcraft`): one command catalog drives the GUI, the CLI, and the MCP server.

- **MCP**: use the shared **`craft-mcp-server`** crate from `storytold/craft-libs` (same git `rev` as
  the other craft-libs deps; only depends on `serde_json`). It implements JSON-RPC 2.0 over stdio
  (newline-delimited), the MCP lifecycle, tools/resources/prompts, tool schemas and argument checks,
  and text/image results. The app implements its `Tools` trait; `Server` speaks the protocol.
  (`wordcraft-mcp` is the reference: a small crate wrapping the engine in a `Tools` impl.)
- **CLI**: ArtCraft does **not** use `clap`. `wordcraft-cli` is a hand-rolled subcommand parser
  (`USAGE` string + `OptionSpec`/`validate_options`, positional + `--flag value` handling) that
  dispatches to the command registry with `serde_json` params, exiting 0/1/3. Follow the same pattern
  (or justify `clap` if the command set makes hand-rolling unwieldy — ArtCraft's choice is
  hand-rolled).
- A JSON control channel to drive a running app is optional; `craft`'s `automation` feature provides
  it (WordCraft's `--connect` is still refused until it confines file access).

## Refs

- `architecture.md` §11 · `CODE_STRUCTURE.md` (apps) · `CODINGSTANDARDS-RUST.md`

## Tests-first (Red)

Write failing tests that drive a command via each surface (CLI arg, JSON control message, MCP call)
and assert the same domain effect as the GUI `Action`.

## Implement (Green → Refactor)

1. Add `craft-mcp-server` (git-pinned) to `[workspace.dependencies]`.
2. `crates/rivercrossing-mcp`: implement `craft_mcp_server::Tools` over the `Action` registry (one tool
   per command, schemas from the registry), then `Server::new(tools).serve(stdin, stdout)`.
3. `apps/rivercrossing-cli`: a hand-rolled subcommand parser (ArtCraft style) — `commands`, `run`,
   `mcp`, and the inspect/convert equivalents that apply — dispatching to the same registry via
   `serde_json`.
4. (Optional) a JSON control channel via `craft`'s `automation` feature for a running app.
5. SPDX line on every `.rs`.

## Done when

- CLI + MCP execute the same commands as the GUI; the full `scripts/validate-rust.sh` gate green.
- Update `docs/ui-porting-runbook.md` with the automation-surface lessons.
