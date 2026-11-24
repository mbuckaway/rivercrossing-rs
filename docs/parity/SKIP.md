# Tier C parity — EPIC-10 SKIP

- **Host:** `markhomemac` (macOS), oracle repo `/Users/mark/src/rivercrossing`
- **Oracle:** Python 3.14 / wxPython 4.3.1
- **Date:** 2026-11-24
- **Rust baseline:** `c8def8d` (EPIC-10 PR 1/2, the live control channel)
- **Reason:** Tier C runs at EPIC-12 exit, when the screens exist. EPIC-10 ships the v1 schema,
  the Rust scenario model, the Tier B control smoke and the in-process Python driver stub; there is
  no screen to compare yet.
- **Re-run when:** the EPIC-12 screen suite lands. Remove this file when the parity run is green.
- **Allowed skip triggers:** the oracle repo/path is absent; the Python or wxPython version does not
  match; the host is unreachable. Anything else is a failure, not a skip.

The wire contract is `tools/parity/schema.json`; the driver stub is
`tools/parity/python/parity_driver.py` (stdlib only, no `wx`).
