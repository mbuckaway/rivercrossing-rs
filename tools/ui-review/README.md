# UI review capture

Dev-only tool for the mockup comparison. It drives the **debug** build through the control
channel, takes screenshots, and copies them out of `target/control/` into
`design/screenshots/review/`.

## Build and run

```bash
cargo build -p rivercrossing --locked          # debug: the release build ignores --control
target/debug/RiverCrossing                     # the bin is capitalised for the macOS app menu
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

Steps: `command` (`ui.command`), `try_command` (same, errors ignored — used by the reset prefix),
`click` (`ui.click` with a selector), `set` (`ui.set`), `dialog_set` (`ui.dialog.set`),
`dialog_confirm` / `dialog_cancel`, `try_dialog_confirm` (ignored when no dialog is open),
`wait` (seconds), `shot` (`ui.screenshot` then copy; a same-size resize first so an idle window
presents a frame). A recipe that fails is reported and the sweep continues.

Every recipe starts with a reset prefix: stop, finish and clear any persisted active ride, answering
each confirmation. A ride-dependent recipe then creates a ride, runs the simulator (Generate Riders,
then GO) and resumes it where a correction dialog needs a live ride.

## Recipes present

Console: `console-no-ride`, `console-draft`, `console-finished`, `sim-running`.
Dialogs: `settings`, `ride-setup`, `ride-library`, `standings-no-ride`, `standings-full`, `audit`,
`simulation`, `shortcuts`, `self-test`, `about`, `edit-crossing`, `manual-deal`, `dnf`, `set-start`.

Dialogs that still need a recipe: crossing detail (needs a feed-row click), void card (from the
detail), delete ride (library row selection), resume (launch with a prior running ride), publish
(needs a WordPress config), and the roster editors (need rider/team creation steps).

## What this is not

Not a test. It is not part of `nextest`, the gate, or the product binary. The `-P ui` goldens remain
the regression evidence; this tool produces the visual review evidence.
