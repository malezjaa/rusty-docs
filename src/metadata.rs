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

impl Package {
    pub fn library_target(&self) -> Option<&Target> {
        self.targets.iter().find(|target| {
            target.kind.iter().any(|kind| {
                matches!(
                    kind.as_str(),
                    "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro"
                )
            })
        })
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
    let libraries = metadata
        .packages
        .iter()
        .filter(|package| members.contains(&package.id) && package.library_target().is_some())
        .cloned()
        .collect::<Vec<_>>();

    if all_workspace {
        if libraries.is_empty() {
            bail!("This Cargo workspace has no library packages to document.");
        }
        return Ok(libraries);
    }

    if let Some(name) = requested.or(configured) {
        return find_package(&libraries, name);
    }

    if let Some(path) = manifest_path
        && let Some(package) = package_at_manifest(&libraries, path)
    {
        return Ok(vec![package]);
    }

    let current_dir = std::env::current_dir().context("Could not read the current directory")?;
    if let Some(package) = deepest_package_for_dir(&libraries, &current_dir) {
        return Ok(vec![package]);
    }

    let defaults = metadata
        .workspace_default_members
        .iter()
        .collect::<HashSet<_>>();
    let default_libraries = libraries
        .iter()
        .filter(|package| defaults.contains(&package.id))
        .cloned()
        .collect::<Vec<_>>();
    if default_libraries.len() == 1 {
        return Ok(default_libraries);
    }
    if libraries.len() == 1 {
        return Ok(libraries);
    }
    if libraries.is_empty() {
        bail!("No library packages were found in this Cargo workspace.");
    }

    choose_interactively(&libraries)
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
            bail!("No library package named '{name}' was found. Available packages: {choices}");
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
            "More than one library package is available. Choose one with --package, or use --workspace. Available packages: {choices}"
        );
    }

    let choices = packages
        .iter()
        .map(|package| format!("{} v{}", package.name, package.version))
        .collect::<Vec<_>>();
    let defaults = vec![true; choices.len()];
    let selections = MultiSelect::new()
        .with_prompt("Select library packages to document")
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
