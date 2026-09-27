use std::{
    collections::HashSet,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use dialoguer::MultiSelect;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CargoMetadata {
    pub packages: Vec<Package>,
    pub workspace_members: Vec<String>,
    #[serde(default)]
    pub workspace_default_members: Vec<String>,
    pub workspace_root: PathBuf,
    pub target_directory: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub manifest_path: PathBuf,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    pub targets: Vec<Target>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Target {
    pub name: String,
    pub kind: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetKind {
    Library,
    Binary,
}

impl TargetKind {
    pub fn is_library(self) -> bool {
        self == Self::Library
    }

    pub fn is_binary(self) -> bool {
        self == Self::Binary
    }
}

impl Package {
    pub fn documentable_targets(&self) -> impl Iterator<Item = (&Target, TargetKind)> {
        self.targets.iter().filter_map(|target| {
            let kind = if target.kind.iter().any(|kind| kind == "bin") {
                TargetKind::Binary
            } else if target.kind.iter().any(|kind| {
                matches!(
                    kind.as_str(),
                    "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro"
                )
            }) {
                TargetKind::Library
            } else {
                return None;
            };
            Some((target, kind))
        })
    }

    pub fn has_documentable_targets(&self) -> bool {
        self.documentable_targets().next().is_some()
    }

    pub fn root(&self) -> &Path {
        self.manifest_path
            .parent()
            .expect("Cargo package manifests have a parent")
    }
}

pub fn load(manifest_path: Option<&Path>) -> Result<CargoMetadata> {
    let mut command = Command::new("cargo");
    command.args(["metadata", "--format-version", "1", "--no-deps"]);
    if let Some(path) = manifest_path {
        command.arg("--manifest-path").arg(path);
    }

    let output = command
        .output()
        .context("Could not run cargo metadata. Install Rust and Cargo first.")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("cargo metadata failed:\n{}", stderr.trim());
    }

    serde_json::from_slice(&output.stdout).context("Cargo returned invalid metadata JSON")
}

pub fn select_packages(
    metadata: &CargoMetadata,
    requested: Option<&str>,
    configured: Option<&str>,
    all_workspace: bool,
    manifest_path: Option<&Path>,
) -> Result<Vec<Package>> {
    let members = metadata.workspace_members.iter().collect::<HashSet<_>>();
    let packages = metadata
        .packages
        .iter()
        .filter(|package| members.contains(&package.id) && package.has_documentable_targets())
        .cloned()
        .collect::<Vec<_>>();

    if all_workspace {
        if packages.is_empty() {
            bail!("This Cargo workspace has no library or binary targets to document.");
        }
        return Ok(packages);
    }

    if let Some(name) = requested.or(configured) {
        return find_package(&packages, name);
    }

    if let Some(path) = manifest_path {
        if packages.len() > 1 && is_workspace_manifest(metadata, path) {
            return choose_interactively(&packages);
        }
        if let Some(package) = package_at_manifest(&packages, path) {
            return Ok(vec![package]);
        }
    }

    let current_dir = std::env::current_dir().context("Could not read the current directory")?;
    if packages.len() > 1 && normalize(&current_dir) == normalize(&metadata.workspace_root) {
        return choose_interactively(&packages);
    }
    if let Some(package) = deepest_package_for_dir(&packages, &current_dir) {
        return Ok(vec![package]);
    }

    let defaults = metadata
        .workspace_default_members
        .iter()
        .collect::<HashSet<_>>();
    let default_packages = packages
        .iter()
        .filter(|package| defaults.contains(&package.id))
        .cloned()
        .collect::<Vec<_>>();
    if default_packages.len() == 1 {
        return Ok(default_packages);
    }
    if packages.len() == 1 {
        return Ok(packages);
    }
    if packages.is_empty() {
        bail!("No library or binary targets were found in this Cargo workspace.");
    }

    choose_interactively(&packages)
}

fn find_package(packages: &[Package], name: &str) -> Result<Vec<Package>> {
    match packages.iter().find(|package| package.name == name) {
        Some(package) => Ok(vec![package.clone()]),
        None => {
            let choices = packages
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "No package with library or binary targets named '{name}' was found. Available packages: {choices}"
            );
        }
    }
}

fn package_at_manifest(packages: &[Package], path: &Path) -> Option<Package> {
    let manifest = normalize(path);
    packages
        .iter()
        .find(|package| normalize(&package.manifest_path) == manifest)
        .cloned()
}

fn is_workspace_manifest(metadata: &CargoMetadata, path: &Path) -> bool {
    normalize(path) == normalize(&metadata.workspace_root.join("Cargo.toml"))
}

fn deepest_package_for_dir(packages: &[Package], dir: &Path) -> Option<Package> {
    let dir = normalize(dir);
    packages
        .iter()
        .filter(|package| dir.starts_with(normalize(package.root())))
        .max_by_key(|package| package.root().components().count())
        .cloned()
}

fn normalize(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn choose_interactively(packages: &[Package]) -> Result<Vec<Package>> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        let choices = packages
            .iter()
            .map(|package| package.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "More than one documentable package is available. Choose one with --package, or use --workspace. Available packages: {choices}"
        );
    }

    let choices = packages
        .iter()
        .map(|package| format!("{} v{}", package.name, package.version))
        .collect::<Vec<_>>();
    let defaults = vec![true; choices.len()];
    let selections = MultiSelect::new()
        .with_prompt("Select packages to document")
        .items(&choices)
        .defaults(&defaults)
        .interact_opt()
        .context("Could not read package selection")?
        .ok_or_else(|| anyhow::anyhow!("Package selection cancelled."))?;
    Ok(selections
        .into_iter()
        .map(|selection| packages[selection].clone())
        .collect())
}
