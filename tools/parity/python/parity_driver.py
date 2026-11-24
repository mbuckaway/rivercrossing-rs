#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""The Tier C Python parity driver stub.

Tier C runs at EPIC-12 exit, on the Mac, inside the Python oracle repo. wxPython's
``FindWindowByName``, ``SetValue`` and posted ``wx.CommandEvent`` cannot be driven from an
external CLI, so the driver is imported in-process by the pytest module. This stub ships the
wire contract and a runnable schema/observation check; it deliberately does not import ``wx``.

The shared contract is ``tools/parity/schema.json``. A missing prerequisite is recorded in
``docs/parity/SKIP.md`` with its exact reason; it is never reported as a pass.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[3]
SCHEMA = ROOT / "tools" / "parity" / "schema.json"
SCENARIO = ROOT / "tools" / "parity" / "scenarios" / "smoke.json"


def load_json(path: Path) -> Any:
    """Read a JSON file from *path*."""
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def validate_scenario(scenario: dict[str, Any]) -> None:
    """Check the v1 scenario's required keys and step kinds without a schema library."""
    if scenario.get("version") != 1:
        raise ValueError("the scenario version must be 1")
    if not isinstance(scenario.get("id"), str):
        raise ValueError("the scenario id must be a string")
    steps = scenario.get("steps")
    if not isinstance(steps, list):
        raise ValueError("the scenario steps must be a list")
    kinds = {"command", "click", "type", "key", "snapshot", "expect"}
    for step in steps:
        if not isinstance(step, dict) or step.get("kind") not in kinds:
            raise ValueError(f"unknown step kind: {step!r}")


def observation(app: str, viewport: tuple[int, int], nodes: list[dict[str, Any]]) -> dict[str, Any]:
    """Build a v1 observation around *nodes*."""
    return {
        "app": app,
        "version": 1,
        "viewport": [viewport[0], viewport[1]],
        "tree": {
            "nodes": nodes,
            "focus": nodes[0]["id"] if nodes else None,
            "root": nodes[0]["id"] if nodes else None,
        },
        "screenshots": [],
    }


def main(argv: list[str]) -> int:
    """Validate the shipped scenario and schema, or record the Tier C SKIP."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="validate the shipped scenario")
    parser.add_argument("--out", type=Path, help="write a sample observation to this path")
    args = parser.parse_args(argv)

    schema = load_json(SCHEMA)
    scenario = load_json(SCENARIO)
    validate_scenario(scenario)
    if "Scenario" not in schema.get("$defs", {}):
        raise ValueError("the schema has no Scenario definition")
    if args.out is not None:
        args.out.write_text(
            json.dumps(observation("python-oracle", (1100, 780), []), indent=2) + "\n",
            encoding="utf-8",
        )
    if args.check:
        print(f"scenario {scenario['id']} is valid")
        return 0
    print("SKIP: Tier C runs on the Mac at EPIC-12 exit; see docs/parity/SKIP.md")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
