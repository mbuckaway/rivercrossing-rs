# Tier C parity — EPIC-12 close-out SKIP

- **Host:** `markhomemac` (macOS), oracle repo `/Users/mark/src/rivercrossing`
- **Oracle:** Python 3.14 / wxPython 4.3.1
- **Date:** 2026-11-25
- **Rust baseline:** `75dc30a` (EPIC-12d merged; the 12e close-out branch follows it)
- **Reason:** the screen suite now exists (30 windows, 30 goldens, the 53-action registry), so the
  12e close-out is the point where Tier C would run. This box cannot run it: the parity driver needs
  the Mac's Aqua session and the Python/wxPython oracle checkout, and Tier C has never been part of
  the gate. **No Tier C run has been performed; no parity result is claimed.**
- **Re-run when:** an operator has the Mac desktop session and the oracle checkout. Remove this file
  only when that run is green.
- **Allowed skip triggers:** the oracle repo/path is absent; the Python or wxPython version does not
  match; the host is unreachable. Anything else is a failure, not a skip.

The wire contract is `tools/parity/schema.json`; the scenarios live in `tools/parity/scenarios/`;
the driver stub is `tools/parity/python/parity_driver.py` (stdlib only, no `wx`). The Rust side it
drives is the control channel (`cargo nextest -P functional`'s `control_channel_smoke` exercises the
same `ui.accessTree`/`ui.command`/`ui.dialog.*` surface).
