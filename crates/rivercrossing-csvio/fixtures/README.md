# `rivercrossing-csvio` fixtures

Input CSVs and byte-goldens for the roster and standings CSV contract (R-21, R-24, R-65).

All inputs are LF, UTF-8, no BOM. The goldens under `golden/` are CRLF by contract and are marked
`-text` in `.gitattributes` so git never normalises them.

## Provenance

| File | Bytes | sha256 | Source |
|---|---|---|---|
| `clean_180.csv` | 5255 | `f436e388ffc03591e8a17e1c1c45ccdf0824cf0da14e90600ac064c36197f3a7` | Python project's synthetic test fixture (`tests/unit/fixtures/csv/clean_180.csv`) |
| `clean_pooled.csv` | 300 | `9b98a5349a972750c42ba9535fc056638ad4a5ce8ea9a0092161908938902ff1` | same, verbatim |
| `dup_plate.csv` | 88 | `33796ddae32c8413c9665f95aa00e499e5ef88219dfa65f59628823bb1c0fda6` | same, verbatim |
| `missing_name.csv` | 63 | `c9eab436dd596c004d4e7f6308f72ca77c1d988a71bc177da71b175c3bd43cf4` | same, verbatim |
| `team_over_max.csv` | 179 | `98b2c2e15caa34499c6b00388342931e85a6862f73bb408e41e4722beba7b09e` | same, verbatim |
| `team_under_min_pooled.csv` | 163 | `0da51a2b3dc4f2b7689367e8e9048e399a9ff31925696e20cd47039694c3e6ff` | same, verbatim |
| `gorba_fake.csv` | 1362 | `f920dfcfa31dcc97dca5622c3f03cdd022a470da1316190a7b2f07bb414f1a00` | same, verbatim; synthetic names |
| `gorba_epic_scrubbed.csv` | 11538 | `2ccb9e0d8c7022486307ef98ed00771a183742cf267eaa8df0e3d0fc472a2e53` | scrubbed derivative of the Python project's real GORBA registration export |

The seven verbatim files and `gorba_fake.csv` are synthetic Python test data and contain no personal
data.

### The scrubbed GORBA fixture

The Python project's `gorba_epic.csv` is a verbatim event registration export: real names, phone
numbers, emergency contacts and medical disclosures. It is **not** in this repository and must not
be added. `scripts/scrub-gorba-fixture.py` produced `gorba_epic_scrubbed.csv` from it, replacing
identifying and health cells with salt-keyed synthetic values while preserving row order, the
12-column header, the quoted comma at line 71, the eight-row footer, team-name whitespace and
punctuation, duplicate counts and the fuzzy-key partition the near-duplicate warning relies on.

The salt was generated for one run and is not retained, so the mapping cannot be reversed; the
fixture is frozen. `--verify` re-runs `rivercrossing.csvio.preview` and refuses to write unless the
pins below still hold. It is a generation-time check, not a CI step (the source file is not in this
repository).

## Pinned outcomes

Pins are `riders · teams · entries · conflicts · warnings` for a mixed, `team_relay` roster:

| File | Pins |
|---|---|
| `clean_180.csv` | 180 · 15 · 135 · 0 · 16 duplicate-name warnings |
| `clean_pooled.csv` | 9 · 2 · 6 · 0 · 0 |
| `dup_plate.csv` | 2 · 0 · 2 · 1 (row 3) · 0 |
| `missing_name.csv` | 0 · 0 · 0 · 1 (row 2) · 0 |
| `team_over_max.csv` | 5 · 1 · 1 · 1 (row 2) · 0 |
| `team_under_min_pooled.csv` | 4 · 2 · 3 · 0 · 1 (row 5) |
| `gorba_fake.csv` | 10 · 5 · 8 · 0 · 5 |
| `gorba_epic_scrubbed.csv` | 101 · 31 · 71 · 0 · 12 |

`gorba_epic_scrubbed.csv` warnings: eight `team-under-min` at rows 17, 36, 41, 45, 59, 84, 85 and
86, plus four near-duplicate-team warnings at rows 13, 20, 37 and 85 (row 85 carries one of each,
so 12 warnings across 11 distinct rows). All 71 entry plates are auto-assigned, numeric and unique.

## Goldens

`golden/` holds the byte-for-byte exporter outputs (`pooled-draft.csv`, `pooled-finished.csv`,
`relay-draft.csv`, `relay-finished.csv`, `standings.csv`, `standings-times.csv`,
`standings-dns.csv`, `formula.csv`). They are CRLF because the Python `csv.writer` writes the
`excel` dialect with CRLF, and the port preserves that byte contract.
