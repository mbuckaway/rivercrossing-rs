# EPIC-13 — CLI + MCP (parity complete)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run last — only after 100% Python parity.

## Goal

Expose the command/Action registry via a CLI, a JSON control channel, and an MCP server, so agents
drive every command the GUI can (mirroring ArtCraft's "every button is a command" surface).

## Context

The `Action` registry built in EPIC-11 is the single source of truth. WordCraft's pattern is the
reference: one command catalog drives the ribbon, keyboard shortcuts, a CLI, a JSON control channel,
and an MCP server. Reuse the `craft` facade's automation/MCP features where available.

## Refs

- `architecture.md` §11 · `CODE_STRUCTURE.md` (apps) · `CODINGSTANDARDS-RUST.md`

## Tests-first (Red)

Write failing tests that drive a command via each surface (CLI arg, JSON control message, MCP call)
and assert the same domain effect as the GUI `Action`.

## Implement (Green → Refactor)

`apps/rivercrossing-cli` (a thin binary over the `Action` registry), the JSON control channel, and the
MCP server. SPDX line on every `.rs`.

## Done when

- CLI + MCP execute the same commands as the GUI; the full `scripts/validate-rust.sh` gate green.
- Update `docs/ui-porting-runbook.md` with the automation-surface lessons.
