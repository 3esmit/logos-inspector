use std::{
    env, fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};

#[must_use]
pub(super) fn path_is_inside(parent: &Path, child: &Path) -> bool {
    match (
        normalized_absolute_path(parent),
        normalized_absolute_path(child),
    ) {
        (Ok(parent), Ok(child)) => child != parent && child.starts_with(parent),
        _ => false,
    }
}

pub(super) fn remove_dir_inside(root: &Path, target: &Path) -> Result<()> {
    if !path_is_inside(root, target) {
        bail!(
            "refusing to remove {} because it is outside managed workspace {}",
            target.display(),
            root.display()
        );
    }
    if target.exists() {
        fs::remove_dir_all(target)
            .with_context(|| format!("failed to remove {}", target.display()))?;
    }
    Ok(())
}

fn normalized_absolute_path(path: &Path) -> Result<PathBuf> {
    let mut normalized = if path.is_absolute() {
        PathBuf::new()
    } else {
        env::current_dir().context("failed to read current directory")?
    };
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    bail!("path {} escapes filesystem root", path.display());
                }
            }
            Component::Normal(value) => normalized.push(value),
        }
    }
    Ok(normalized)
}
