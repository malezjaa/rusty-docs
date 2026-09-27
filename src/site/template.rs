use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use include_dir::{Dir, include_dir};

static TEMPLATE: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates/site");
const MARKER: &str = ".rusty-docs-managed";
const CURRENT_TEMPLATE_VERSION: u32 = 0;

#[derive(Clone, Copy)]
enum CopyPolicy {
    Overwrite,
    KeepExisting,
}

pub(super) fn ensure_template(site_dir: &Path) -> Result<()> {
    let marker = site_dir.join(MARKER);
    if marker.is_file() {
        let version = managed_version(&marker)?;
        if let Some(version) = version
            && version > CURRENT_TEMPLATE_VERSION
        {
            bail!(
                "Generated site {} uses template version {}, but this rusty-docs build supports up to version {}",
                site_dir.display(),
                version,
                CURRENT_TEMPLATE_VERSION
            );
        }

        let policy = if version == Some(CURRENT_TEMPLATE_VERSION) {
            CopyPolicy::KeepExisting
        } else {
            CopyPolicy::Overwrite
        };
        copy_embedded_dir(&TEMPLATE, site_dir, policy)?;
        if version != Some(CURRENT_TEMPLATE_VERSION) {
            write_marker(&marker)?;
        }
        return Ok(());
    }

    if site_dir.exists() {
        let mut entries = fs::read_dir(site_dir).with_context(|| {
            format!("Could not inspect output directory {}", site_dir.display())
        })?;
        if entries.next().is_some() {
            bail!(
                "Output directory {} already exists and was not created by rusty-docs. Choose another --out-dir or move the existing files.",
                site_dir.display()
            );
        }
    }

    fs::create_dir_all(site_dir)
        .with_context(|| format!("Could not create output directory {}", site_dir.display()))?;
    copy_embedded_dir(&TEMPLATE, site_dir, CopyPolicy::Overwrite)?;
    write_marker(&marker)
}

fn managed_version(marker: &Path) -> Result<Option<u32>> {
    let contents = fs::read_to_string(marker)
        .with_context(|| format!("Could not read {}", marker.display()))?;
    parse_managed_version(&contents)
        .with_context(|| format!("Could not parse template version in {}", marker.display()))
}

fn parse_managed_version(contents: &str) -> Result<Option<u32>> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix("Version: "))
        .map(str::parse)
        .transpose()
        .context("Template version must be an unsigned integer")
}

fn write_marker(marker: &Path) -> Result<()> {
    let contents = format!("Managed by rusty-docs.\nVersion: {CURRENT_TEMPLATE_VERSION}\n");
    fs::write(marker, contents).with_context(|| format!("Could not write {}", marker.display()))
}

fn copy_embedded_dir(source: &Dir<'_>, destination: &Path, policy: CopyPolicy) -> Result<()> {
    for file in source.files() {
        let path = destination.join(file.path());
        if path.exists() && matches!(policy, CopyPolicy::KeepExisting) {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Could not create directory {}", parent.display()))?;
        }
        fs::write(&path, file.contents())
            .with_context(|| format!("Could not write template file {}", path.display()))?;
    }
    for directory in source.dirs() {
        let path = destination.join(directory.path());
        fs::create_dir_all(&path)
            .with_context(|| format!("Could not create directory {}", path.display()))?;
        copy_embedded_dir(directory, destination, policy)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_current_baseline_version() {
        assert_eq!(
            parse_managed_version("Managed by rusty-docs.\nVersion: 0\n").unwrap(),
            Some(0)
        );
    }

    #[test]
    fn treats_legacy_markers_as_pre_baseline() {
        assert_eq!(
            parse_managed_version("Managed by rusty-docs.\n").unwrap(),
            None
        );
    }

    #[test]
    fn rejects_invalid_versions() {
        assert!(parse_managed_version("Version: next\n").is_err());
    }
}
