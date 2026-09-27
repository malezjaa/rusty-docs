use std::{fs, path::Path};

use anyhow::{Context, Result};

pub(super) fn sync_content(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination).with_context(|| {
            format!(
                "Could not clear managed content directory {}",
                destination.display()
            )
        })?;
    }
    fs::create_dir_all(destination)
        .with_context(|| format!("Could not create {}", destination.display()))?;
    if !source.is_dir() {
        return Ok(());
    }
    copy_content_tree(source, &destination.join("content"))
}

fn copy_content_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("Could not create {}", destination.display()))?;
    let mut entries = fs::read_dir(source)
        .with_context(|| format!("Could not read content directory {}", source.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_type = entry.file_type()?;
        if file_type.is_symlink() || entry.file_name() == ".git" {
            continue;
        }
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_content_tree(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target).with_context(|| {
                format!(
                    "Could not copy {} to {}",
                    entry.path().display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}
