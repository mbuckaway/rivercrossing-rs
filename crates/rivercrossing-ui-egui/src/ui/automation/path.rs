// SPDX-License-Identifier: GPL-3.0-only

//! The screenshot write authority: one canonical control root under `target/control`.

use std::path::{Component, Path, PathBuf};

/// Create `<current_dir>/target/control` and return its canonical path.
///
/// # Errors
///
/// Return the I/O error when the current directory cannot be read or the root cannot be created.
pub(crate) fn write_root() -> std::io::Result<PathBuf> {
    let root = std::env::current_dir()?.join("target").join("control");
    std::fs::create_dir_all(&root)?;
    root.canonicalize()
}

/// Write `bytes` to `name` under `root`, refusing every path that leaves the root.
///
/// # Errors
///
/// Return an error when `name` is empty, absolute, has a `..`/root/prefix component, is a
/// symlink, resolves outside `root`, or the write fails.
pub(crate) fn write_screenshot(root: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let relative = Path::new(name);
    if relative.as_os_str().is_empty() {
        return Err("screenshot path is empty".to_owned());
    }
    if relative.is_absolute() {
        return Err(format!("screenshot path `{name}` must be relative"));
    }
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!(
            "screenshot path `{name}` must stay inside the control directory"
        ));
    }
    let target = root.join(relative);
    match std::fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!("screenshot path `{name}` is a symlink"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect `{name}`: {error}")),
    }
    let parent = target
        .parent()
        .ok_or_else(|| format!("screenshot path `{name}` has no parent"))?;
    let parent = parent
        .canonicalize()
        .map_err(|error| format!("cannot resolve `{name}`: {error}"))?;
    if !parent.starts_with(root) {
        return Err(format!(
            "screenshot path `{name}` escapes the control directory"
        ));
    }
    std::fs::write(&target, bytes).map_err(|error| format!("cannot write `{name}`: {error}"))?;
    Ok(target)
}
