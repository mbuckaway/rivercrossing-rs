#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Capture app screens through the control channel for mockup comparison.

Dev-only tool: it launches the DEBUG binary (the release build ignores --control),
authenticates on the loopback control port, runs a small recipe of control-channel
calls, and copies each screenshot out of `target/control/` into the review folder.

Usage:
    python3 tools/ui-review/capture.py --recipe tools/ui-review/recipes/console-no-ride.json
    python3 tools/ui-review/capture.py --all

A recipe is JSON: {"screen": "console-no-ride", "theme": "light", "steps": [...]}.
Each step is one of:
    {"command": "<action id>"}          ui.command {"name": ...}
    {"click": {"label": "Riders"}}      ui.click {"selector": <selector>}
    {"set": {"label": "...", "value": "..."}}   ui.set
    {"wait": 0.3}                       sleep, to let a queued command land
    {"shot": "console-no-ride.png"}     ui.screenshot then copy to --out
The selector is passed through verbatim; see the automation selector type.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import secrets
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DEFAULT_BIN = REPO / "target" / "debug" / "RiverCrossing"
DEFAULT_OUT = REPO / "design" / "screenshots" / "review"
RECIPES = Path(__file__).resolve().parent / "recipes"
LISTEN = re.compile(r"control server listening on ([0-9.]+):(\d+)")


class Channel:
    """One authenticated control-channel connection."""

    def __init__(self, host: str, port: int, token: str) -> None:
        import socket

        self.sock = socket.create_connection((host, port), timeout=30)
        self.stream = self.sock.makefile("rwb")
        self.next_id = 1
        self.call("auth", {"token": token})

    def call(self, method: str, params: dict) -> dict:
        request = {"id": self.next_id, "method": method, "params": params}
        self.next_id += 1
        self.stream.write((json.dumps(request) + "\n").encode())
        self.stream.flush()
        line = self.stream.readline()
        if not line:
            raise RuntimeError(f"the app closed the connection during {method}")
        reply = json.loads(line)
        if not reply.get("ok"):
            raise RuntimeError(f"{method} failed: {reply.get('error')}")
        return reply.get("result")

    def close(self) -> None:
        try:
            self.stream.close()
            self.sock.close()
        except OSError:
            pass


def launch(binary: Path, workspace: Path) -> tuple[subprocess.Popen, str, int]:
    """Start the app and return its process, control host and port."""
    token = secrets.token_hex(32)
    process = subprocess.Popen(
        [str(binary), "--control", "0", "--control-token", token],
        cwd=workspace,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    deadline = time.time() + 60
    while time.time() < deadline:
        line = process.stderr.readline()
        if not line:
            if process.poll() is not None:
                break
            continue
        match = LISTEN.search(line)
        if match:
            return process, match.group(1), int(match.group(2))
    process.kill()
    raise RuntimeError("the control server never announced its port")


def run_recipe(channel: Channel, workspace: Path, recipe: dict, out: Path) -> list[Path]:
    """Run one recipe and return the captured files."""
    captured: list[Path] = []
    screen = recipe.get("screen", "screen")
    for step in recipe.get("steps", []):
        if "command" in step:
            channel.call("ui.command", {"name": step["command"]})
        elif "click" in step:
            channel.call("ui.click", {"selector": step["click"]})
        elif "set" in step:
            channel.call("ui.set", step["set"])
        elif "wait" in step:
            time.sleep(float(step["wait"]))
        elif "shot" in step:
            name = step["shot"]
            channel.call("ui.screenshot", {"path": name})
            source = workspace / "target" / "control" / name
            if not source.is_file():
                raise RuntimeError(f"ui.screenshot did not write {source}")
            suffix = "" if name.startswith(screen) else f"-{Path(name).stem}"
            target = out / f"{screen}{suffix}.png"
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            captured.append(target)
    return captured


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=DEFAULT_BIN)
    parser.add_argument("--workspace", type=Path, default=REPO)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--recipe", type=Path, action="append", default=[])
    parser.add_argument("--all", action="store_true", help="run every recipe in recipes/")
    arguments = parser.parse_args()

    recipes = list(arguments.recipe)
    if arguments.all:
        recipes.extend(sorted(RECIPES.glob("*.json")))
    if not recipes:
        parser.error("give --recipe or --all")
    if not arguments.binary.is_file():
        parser.error(f"no debug binary at {arguments.binary}; run `cargo build -p rivercrossing`")

    process, host, port = launch(arguments.binary, arguments.workspace)
    failures = 0
    try:
        channel = Channel(host, port, token=process.args[-1])
        try:
            for path in recipes:
                recipe = json.loads(path.read_text())
                try:
                    files = run_recipe(channel, arguments.workspace, recipe, arguments.out)
                except Exception as error:  # one bad recipe must not stop the sweep
                    failures += 1
                    print(f"FAIL {path.name}: {error}", file=sys.stderr)
                    continue
                for file in files:
                    print(file.relative_to(REPO))
            try:
                channel.call("ui.command", {"name": "app_quit"})
            except Exception:
                pass
        finally:
            channel.close()
    finally:
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
