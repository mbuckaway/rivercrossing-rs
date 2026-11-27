# UI review capture

Dev-only tool for the mockup comparison. It drives the **debug** build through the control
channel, takes screenshots, and copies them out of `target/control/` into
`design/screenshots/review/`.

## Build and run

```bash
cargo build -p rivercrossing --locked          # debug: the release build ignores --control
python3 tools/ui-review/capture.py --all
python3 tools/ui-review/capture.py --recipe tools/ui-review/recipes/settings.json
```

`--out`, `--workspace` and `--binary` override the defaults. The tool generates a 64-hex token,
launches the app with `--control 0 --control-token <token>`, authenticates on the announced
loopback port, and quits the app when the sweep ends.

## Recipe format

```json
{
  "screen": "settings",
  "steps": [
    { "command": "wxID_PREFERENCES" },
    { "wait": 0.4 },
    { "shot": "settings.png" }
  ]
}
```

Steps: `command` (`ui.command`), `click` (`ui.click` with a selector object), `set` (`ui.set`),
`wait` (seconds), `shot` (`ui.screenshot` then copy). A recipe that fails is reported and the sweep
continues.

## Recipes present

`about`, `console-no-ride`, `ride-library`, `ride-setup`, `self-test`, `settings`, `shortcuts`,
`simulation`, `standings-no-ride`.

Dialogs that need a ride state (audit, crossing detail, corrections, DNF, void, delete, resume,
publishing, simulating) need a recipe that first creates and starts a ride; add them as the
state-driven pass reaches them.

## What this is not

Not a test. It is not part of `nextest`, the gate, or the product binary. The `-P ui` goldens remain
the regression evidence; this tool produces the visual review evidence.
