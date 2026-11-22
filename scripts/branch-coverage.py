#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Fail when summed branch coverage in a cargo-llvm-cov JSON report is below a floor.

cargo-llvm-cov has no `--fail-under-branches`, so the gate reads the JSON totals instead.
Summing every `data` entry mirrors cargo-llvm-cov's own percentage calculation.

Usage: branch-coverage.py <coverage.json> <floor-percent>
Exit: 0 at or above the floor, 1 below it, 2 on a usage or input error.
"""

import json
import sys
from pathlib import Path


def branch_totals(report: dict) -> tuple[int, int]:
    """Return (covered, count) summed over every entry in the report."""
    covered = 0
    count = 0
    for entry in report.get("data", []):
        branches = entry.get("totals", {}).get("branches", {})
        covered += int(branches.get("covered", 0))
        count += int(branches.get("count", 0))
    return covered, count


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: branch-coverage.py <coverage.json> <floor-percent>", file=sys.stderr)
        return 2
    try:
        report = json.loads(Path(argv[1]).read_text(encoding="utf-8"))
        floor = float(argv[2])
    except (OSError, ValueError) as error:
        print(f"branch coverage: cannot read {argv[1]}: {error}", file=sys.stderr)
        return 2

    covered, count = branch_totals(report)
    # A report with no branches measured passes: llvm reports zero regions for a crate with
    # no conditional code, and failing that would penalise an empty tree rather than a risky one.
    percent = covered * 100.0 / count if count else 100.0
    print(f"branch coverage: {covered}/{count} = {percent:.2f}% (floor {floor:.0f}%)")
    return 0 if percent >= floor else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
