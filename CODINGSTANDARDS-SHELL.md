# CODINGSTANDARDS-SHELL.md — rivercrossing-rs

The canonical shell standard is `${HOME}/.agents/standards/CODINGSTANDARDS-SHELL.md`. This file
tightens it for this repo.

- **Bash 3.x only.** No Bash 4.x syntax.
- **No `set -e`.** Check every command's exit code explicitly and fail with a message.
- **ShellCheck-clean**; a ShellCheck violation is build-blocking.
- **Header.** Scripts carry a single `# SPDX-License-Identifier: GPL-3.0-only` line after the shebang
  and no other copyright/licence text.
- **Thin wrappers.** `scripts/*.sh` are one-line wrappers around cargo commands. The only gate is
  `scripts/validate-rust.sh` — run it; never re-implement gate steps inline in another script.
