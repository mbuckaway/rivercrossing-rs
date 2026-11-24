# `tools/parity` — the Python/Rust UI parity contract

One versioned JSON scenario drives both apps. The Rust side is the `ui` test target and the
`src/ui/automation/scenario.rs` model; the Python side is an **in-process** pytest driver in the
oracle repo (`/Users/mark/src/rivercrossing`), because wxPython's `FindWindowByName`, `SetValue`
and posted `wx.CommandEvent` cannot be driven from an external CLI.

## Files

| Path | Role |
|---|---|
| `schema.json` | The v1 wire schema: `Scenario`, `Step`, `Selector`, `Observation`, `WireTree`, `Node` |
| `scenarios/smoke.json` | A minimal scenario proving the shape; it is not a pass/fail parity run |
| `python/parity_driver.py` | The in-process driver stub (stdlib only; no `wx`) |

## Version 1

- `Scenario { version, id, size?, steps }`; `steps` are `command`, `click`, `type`, `key`,
  `snapshot`, `expect` (serde tag `kind`).
- `Selector` is `{ "action": "<frozen id>" }` or `{ "role": "<AccessKit role>", "name": "<label>",
  "window": null }`. The live channel resolves it app-side; a `window` scope is Tier A only at
  EPIC-10.
- `Observation { app, version, viewport, tree, screenshots }`; `tree` mirrors craft's flat
  `ui.accessTree` reply (`nodes`, `focus`, `root`). `textSelection` is deliberately omitted.
- `expect` is the only assertion step: one node must match the selector and its `name`/`value`
  must equal the expected text.

## Running

At EPIC-10 the Rust side is:

```bash
WGPU_BACKEND=vulkan cargo nextest run -P ui -p rivercrossing-ui-egui --locked
```

Tier C runs on the Mac at EPIC-12 exit. A blocked run records `docs/parity/SKIP.md` with the exact
reason (repo/path absent, Python/wx version mismatch, or host unreachable) — anything else fails.
A missing prerequisite is never a pass. "0 structured mismatches" belongs to EPIC-12 exit, when the
screens exist.
