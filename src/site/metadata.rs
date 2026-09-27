use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::rustdoc::ExtractedApi;

pub(super) fn docs_index(title: &str, description: &str, packages: &[ExtractedApi]) -> String {
    let mut output = format!(
        "---\ntitle: {}\ndescription: {}\n---\n\n# {}\n\n{}\n\n",
        yaml_quote("Overview"),
        yaml_quote(description),
        title,
        description
    );
    output.push_str("## API reference\n\n");
    for api in packages {
        output.push_str(&format!(
            "- [`{}` v{}](/docs/api/{})\n",
            api.package.name,
            api.package.version,
            super::util::slug(&api.package.name)
        ));
    }
    output.push_str("\nProject guides are in the Guides section.\n");
    output
}

pub(super) fn api_index(packages: &[ExtractedApi]) -> String {
    let mut output = format!(
        "---\ntitle: {}\ndescription: {}\n---\n\n# API reference\n\n",
        yaml_quote("API reference"),
        yaml_quote("Generated documentation for public Rust APIs.")
    );
    for api in packages {
        output.push_str(&format!(
            "- [`{}` v{}](/docs/api/{})\n",
            api.package.name,
            api.package.version,
            super::util::slug(&api.package.name)
        ));
    }
    output
}

pub(super) fn site_description(packages: &[ExtractedApi]) -> String {
    if packages.len() == 1 {
        return packages[0].package.description.clone().unwrap_or_else(|| {
            format!("API reference and guides for {}.", packages[0].package.name)
        });
    }
    format!(
        "API reference and guides for {} Rust packages.",
        packages.len()
    )
}

pub(super) fn default_title(packages: &[ExtractedApi]) -> String {
    if packages.len() == 1 {
        packages[0].package.name.clone()
    } else {
        "Rust documentation".to_owned()
    }
}

pub(super) fn normalize_base_path(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value == "/" {
        return Ok("/".to_owned());
    }
    if !value.starts_with('/') || value.contains("..") || value.contains('?') || value.contains('#')
    {
        bail!("site.base_path must start with '/' and must not contain '..', '?' or '#'");
    }
    Ok(value.trim_end_matches('/').to_owned())
}

pub(super) fn normalize_site_url(value: &str) -> Result<String> {
    let value = value.trim().trim_end_matches('/');
    if !(value.starts_with("https://") || value.starts_with("http://"))
        || value.contains('?')
        || value.contains('#')
    {
        bail!("site.url must be an http or https origin without a query or fragment");
    }
    let host = value
        .split_once("://")
        .map(|(_, host)| host)
        .unwrap_or_default();
    if host.is_empty() || host.contains('/') {
        bail!("site.url must contain only the origin. Set site.base_path separately.");
    }
    Ok(value.to_owned())
}

pub(super) fn normalize_github_url(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches('/').trim_end_matches(".git");
    let url = value
        .strip_prefix("git+https://")
        .map(|rest| format!("https://{rest}"))
        .or_else(|| value.strip_prefix("https://").map(|_| value.to_owned()))?;
    if !url.starts_with("https://github.com/") {
        return None;
    }
    Some(url)
}

pub(super) fn resolve_from(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

pub(super) fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("Generated JSON path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("Could not create {}", parent.display()))?;
    let contents = serde_json::to_vec_pretty(value).context("Could not serialize site config")?;
    fs::write(path, contents).with_context(|| format!("Could not write {}", path.display()))
}

pub(super) fn write_markdown(path: &Path, contents: &str) -> Result<()> {
    let parent = path
        .parent()
        .context("Generated content path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("Could not create {}", parent.display()))?;
    fs::write(path, contents).with_context(|| format!("Could not write {}", path.display()))
}

pub(super) fn yaml_quote(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
}
