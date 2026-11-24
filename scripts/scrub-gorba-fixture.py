#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Build the scrubbed GORBA CSV fixture for `rivercrossing-csvio`.

The Python project's `tests/unit/fixtures/csv/gorba_epic.csv` is a verbatim
registration export: real names, phone numbers, emergency contacts and medical
disclosures. It must never be committed to the Rust repository. This script
produces a synthetic derivative that keeps every structural property the CSV
contract pins:

* 110 lines: one header, 101 rider rows, eight footer rows;
* the 12-column GORBA header, including the quoted comma in row 71;
* team names keep their whitespace, punctuation, digits, word lengths and the
  exact fuzzy-key partition the near-duplicate warning depends on;
* duplicate values stay duplicated, distinct values stay distinct;
* 71 entries, 40 solos, 31 teams, 0 conflicts, 12 warnings at the same rows.

Names, phones, emergency contacts, medical cells, dates, meal and t-shirt
values are replaced with synthetic values. The mapping is keyed by a secret
salt supplied at run time, so the committed fixture cannot be reversed without
that salt; the salt is deliberately not retained. Run it on the machine that
holds the original (the fixture is a frozen artifact, not a CI step):

    PYTHONPATH=src .venv/bin/python scripts/scrub-gorba-fixture.py \
        --source tests/unit/fixtures/csv/gorba_epic.csv \
        --out /tmp/gorba_epic_scrubbed.csv \
        --salt "$(openssl rand -hex 32)"

The `--verify` pass re-runs `rivercrossing.csvio.preview` over the output and
refuses to write unless every pin matches.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import hmac
import re
import sys
from pathlib import Path
from typing import Iterable

ALPHABET = "bcdfghjklmnpqrstvwxz"

# Column indices in the GORBA export.
COL_DATE = 0
COL_FIRST = 1
COL_LAST = 2
COL_KIND = 3
COL_TEAM = 4
COL_TEAM_SIZE = 5
COL_MEMBERS = 6
COL_MEAL = 7
COL_SHIRT = 8
COL_EMERGENCY_NAME = 9
COL_EMERGENCY_PHONE = 10
COL_MEDICAL = 11

DATA_ROWS = 101
FOOTER_ROWS = 8

MEDICAL_POOL = (
    "none",
    "mild seasonal allergies",
    "asthma",
    "no known conditions",
    "hay fever",
    "migraine",
    "eczema",
    "low blood pressure",
    "recovering ankle sprain",
    "lactose intolerance",
    "mild arthritis",
    "no allergies",
    "tinnitus",
    "iron deficiency",
    "seasonal rhinitis",
    "occasional back pain",
)

QUOTE_TRANSLATION = str.maketrans("", "", "'\"\u2018\u2019\u201c\u201d")
WHITESPACE_TOKEN = re.compile(r"\S+")


class Scrubber:
    """A deterministic, salt-keyed synthetic value generator."""

    def __init__(self, salt: bytes) -> None:
        self._salt = salt
        self._cache: dict[tuple[str, str], str] = {}
        self._used: dict[str, set[str]] = {}

    def _letters(self, parts: Iterable[str], count: int, attempt: int = 0) -> str:
        if count == 0:
            return ""
        payload = "|".join((*parts, str(attempt))).encode()
        out: list[str] = []
        counter = 0
        while len(out) < count:
            digest = hmac.new(self._salt, payload + b"|" + str(counter).encode(), hashlib.sha256).digest()
            out.extend(ALPHABET[byte % len(ALPHABET)] for byte in digest)
            counter += 1
        return "".join(out[:count])

    def _unique(self, column: str, value: str, build) -> str:
        key = (column, value)
        if key in self._cache:
            return self._cache[key]
        used = self._used.setdefault(column, set())
        for attempt in range(10_000):
            candidate = build(attempt)
            if candidate not in used:
                used.add(candidate)
                self._cache[key] = candidate
                return candidate
        raise RuntimeError(f"could not synthesise a unique value for {column!r}")

    def name(self, column: str, value: str) -> str:
        """Replace each letter positionally, preserving length, case and punctuation."""

        def build(attempt: int) -> str:
            letters = self._letters((column, value), sum(char.isalpha() for char in value), attempt)
            out: list[str] = []
            index = 0
            for char in value:
                if char.isalpha():
                    synthetic = letters[index]
                    index += 1
                    out.append(synthetic.upper() if char.isupper() else synthetic.lower())
                else:
                    out.append(char)
            return "".join(out)

        return self._unique(column, value, build)

    def phone(self, value: str) -> str:
        """Keep the punctuation skeleton and digit count; fill with 555 serials."""

        def build(attempt: int) -> str:
            serial = f"555010{attempt:04d}"
            index = 0
            out: list[str] = []
            for char in value:
                if char.isdigit():
                    out.append(serial[index % len(serial)])
                    index += 1
                else:
                    out.append(char)
            return "".join(out)

        return self._unique("phone", value, build)

    def date(self, value: str) -> str:
        def build(attempt: int) -> str:
            return f"2026/09/{1 + attempt % 28:02d} {attempt % 24:02d}:{attempt % 60:02d}:00 PDT"

        return self._unique("date", value, build)

    def medical(self, value: str) -> str:
        if not value.strip():
            return value
        return self._unique(
            "medical",
            value,
            lambda attempt: MEDICAL_POOL[attempt % len(MEDICAL_POOL)]
            + ("" if attempt < len(MEDICAL_POOL) else f" (variant {attempt // len(MEDICAL_POOL)})"),
        )

    def meal(self, value: str, observed: list[str]) -> str:
        if not value.strip():
            return value
        ordered = [item for item in observed if item.strip()]
        unique = list(dict.fromkeys(ordered))
        if len(unique) < 2:
            return value
        return unique[(unique.index(value) + 1) % len(unique)]

    def shirt(self, value: str, observed: list[str]) -> str:
        return self.meal(value, observed)

    def synthetic_team_key(self, key: str, attempt: int = 0) -> str:
        letters = self._letters(("team-key", key), sum(char.isalpha() for char in key), attempt)
        out: list[str] = []
        index = 0
        for char in key:
            if char.isalpha():
                out.append(letters[index])
                index += 1
            else:
                out.append(char)
        return "".join(out)


def kept_positions(name: str) -> list[int]:
    """Return the indices of the characters `team_name_key` keeps.

    The rule is the oracle's: casefold, drop quote characters, split on
    whitespace, drop standalone ``and``/``&`` tokens, keep the remaining
    alphanumerics.
    """
    kept: list[int] = []
    for match in WHITESPACE_TOKEN.finditer(name):
        token = match.group(0)
        folded = token.casefold().translate(QUOTE_TRANSLATION)
        if folded in ("and", "&"):
            continue
        for offset, char in enumerate(token):
            if char.isalnum():
                kept.append(match.start() + offset)
    return kept


def rebuild_team(name: str, synthetic_key: str) -> str:
    positions = kept_positions(name)
    if len(positions) != len(synthetic_key):
        raise ValueError(
            f"team {name!r} keeps {len(positions)} characters but its key is {synthetic_key!r}"
        )
    replacements = dict(zip(positions, synthetic_key))
    out: list[str] = []
    for index, char in enumerate(name):
        replacement = replacements.get(index)
        if replacement is None:
            out.append(char)
        else:
            out.append(replacement.upper() if char.isupper() else replacement.lower())
    return "".join(out)


def scrub_team_names(rows: list[list[str]], scrubber: Scrubber, team_name_key) -> list[list[str]]:
    groups: dict[str, list[str]] = {}
    for row in rows:
        name = row[COL_TEAM]
        if name.strip():
            groups.setdefault(team_name_key(name), []).append(name)

    synthetic_keys: dict[str, str] = {}
    used_keys: set[str] = set()
    for key in groups:
        attempt = 0
        candidate = scrubber.synthetic_team_key(key, attempt)
        while candidate in used_keys:
            attempt += 1
            candidate = scrubber.synthetic_team_key(key, attempt)
        used_keys.add(candidate)
        synthetic_keys[key] = candidate

    for row in rows:
        name = row[COL_TEAM]
        if name.strip():
            row[COL_TEAM] = rebuild_team(name, synthetic_keys[team_name_key(name)])

    for key, members in groups.items():
        rebuilt = {rebuild_team(member, synthetic_keys[key]) for member in members}
        if len(rebuilt) != len(set(members)):
            raise ValueError(f"team group {key!r} collapsed after scrubbing")
    return rows


def load_rows(path: Path) -> list[list[str]]:
    with path.open("r", encoding="utf-8-sig", newline="") as handle:
        return list(csv.reader(handle))


def write_rows(path: Path, rows: list[list[str]]) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerows(rows)


def verify(path: Path) -> None:
    from rivercrossing.csvio import preview
    from rivercrossing.roster import EntryMode, PlateModel, Roster

    roster = Roster(entry_mode=EntryMode.MIXED, plate_model=PlateModel.TEAM_RELAY)
    result = preview(path, roster)

    if result.conflicts:
        raise AssertionError(f"conflicts: {result.conflicts!r}")
    under_min = tuple(w.row for w in result.warnings if "team-under-min" in w.problem)
    if under_min != (17, 36, 41, 45, 59, 84, 85, 86):
        raise AssertionError(f"under-min rows: {under_min!r}")
    near_dup = tuple(w.row for w in result.warnings if w.problem.startswith("possible duplicate"))
    if near_dup != (13, 20, 37, 85):
        raise AssertionError(f"near-duplicate rows: {near_dup!r}")
    pins = {
        "warnings": (len(result.warnings), 12),
        "riders": (result.rider_count, 101),
        "teams": (result.team_count, 31),
        "entries": (len(result.entries), 71),
    }
    for label, (actual, expected) in pins.items():
        if actual != expected:
            raise AssertionError(f"{label}: {actual} != {expected}")
    solo = [entry for entry in result.entries if entry.type.name == "SOLO"]
    if len(solo) != 40:
        raise AssertionError(f"solo entries: {len(solo)}")
    plates = [entry.plate for entry in result.entries]
    if not all(plate.isdigit() for plate in plates) or len(set(plates)) != 71:
        raise AssertionError("entry plates are not 71 unique digit strings")
    print(
        "verified: 101 riders · 31 teams · 71 entries · 40 solos · 0 conflicts · 12 warnings"
    )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--salt", required=True, help="secret hex string; not retained")
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args(argv)

    if args.verify_only:
        verify(args.out)
        return 0

    from rivercrossing.roster import team_name_key

    rows = load_rows(args.source)
    if len(rows) != 1 + DATA_ROWS + FOOTER_ROWS:
        raise SystemExit(f"unexpected row count: {len(rows)}")
    header, data, footer = rows[0], rows[1 : 1 + DATA_ROWS], rows[1 + DATA_ROWS :]

    scrubber = Scrubber(bytes.fromhex(args.salt))

    meals = [row[COL_MEAL] for row in data]
    shirts = [row[COL_SHIRT] for row in data]
    for row in data:
        row[COL_DATE] = scrubber.date(row[COL_DATE])
        row[COL_FIRST] = scrubber.name("first", row[COL_FIRST])
        row[COL_LAST] = scrubber.name("last", row[COL_LAST])
        row[COL_MEMBERS] = scrubber.name("members", row[COL_MEMBERS])
        row[COL_EMERGENCY_NAME] = scrubber.name("emergency", row[COL_EMERGENCY_NAME])
        row[COL_EMERGENCY_PHONE] = scrubber.phone(row[COL_EMERGENCY_PHONE])
        row[COL_MEDICAL] = scrubber.medical(row[COL_MEDICAL])
        row[COL_MEAL] = scrubber.meal(row[COL_MEAL], meals)
        row[COL_SHIRT] = scrubber.shirt(row[COL_SHIRT], shirts)

    scrub_team_names(data, scrubber, team_name_key)

    out_rows = [header, *data, *footer]
    write_rows(args.out, out_rows)
    verify(args.out)
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
