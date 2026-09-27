use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub site: SiteOptions,
    pub api: ApiOptions,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct SiteOptions {
    pub title: Option<String>,
    pub url: Option<String>,
    pub content_dir: PathBuf,
    pub output_dir: PathBuf,
    pub base_path: String,
}

impl Default for SiteOptions {
    fn default() -> Self {
        Self {
            title: None,
            url: None,
            content_dir: PathBuf::from("docs"),
            output_dir: PathBuf::from("docs-site"),
            base_path: "/".to_owned(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ApiOptions {
    pub package: Option<String>,
    pub features: Vec<String>,
    pub all_features: bool,
    pub no_default_features: bool,
}

pub fn load(path: Option<PathBuf>, workspace_root: &std::path::Path) -> Result<Config> {
    let path = match path {
        Some(path) if path.is_absolute() => path,
        Some(path) => std::env::current_dir()
            .context("Could not read the current directory")?
            .join(path),
        None => workspace_root.join("rusty-docs.toml"),
    };

    if !path.exists() {
        if path
            .file_name()
            .is_some_and(|name| name == "rusty-docs.toml")
        {
            return Ok(Config::default());
        }
        anyhow::bail!("Configuration file does not exist: {}", path.display());
    }

    let input = fs::read_to_string(&path)
        .with_context(|| format!("Could not read configuration file {}", path.display()))?;
    toml::from_str(&input)
        .with_context(|| format!("Could not parse configuration file {}", path.display()))
}
