use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::rustdoc::ExtractedTarget;

pub(super) fn docs_index(title: &str, description: &str, targets: &[ExtractedTarget]) -> String {
    let mut output = format!(
        "---\ntitle: {}\ndescription: {}\n---\n\n# {}\n\n{}\n\n",
        yaml_quote("Overview"),
        yaml_quote(description),
        title,
        description
    );
    let libraries = targets
        .iter()
        .filter(|target| target.target_kind.is_library())
        .collect::<Vec<_>>();
    if !libraries.is_empty() {
        output.push_str("## Library APIs\n\n");
    }
    for api in libraries {
        output.push_str(&format!(
            "- [`{}` v{}](/docs/api/{})\n",
            api.package.name,
            api.package.version,
            super::util::slug(&api.package.name)
        ));
    }
    let binaries = targets
        .iter()
        .filter(|target| target.target_kind.is_binary())
        .collect::<Vec<_>>();
    if !binaries.is_empty() {
        output.push_str("\n## Binary source references\n\n");
    }
    for target in binaries {
        output.push_str(&format!(
            "- [`{}` from `{}`]({})\n",
            target.target_name,
            target.package.name,
            super::util::page_route(&target.route_root.join("index.mdx"))
        ));
    }
    output.push_str("\nAdditional documentation pages are available in the sidebar.\n");
    output
}

pub(super) fn api_index(targets: &[ExtractedTarget]) -> String {
    let mut output = format!(
        "---\ntitle: {}\ndescription: {}\n---\n\n# API\n\n",
        yaml_quote("API"),
        yaml_quote("Generated documentation for public Rust APIs.")
    );
    let libraries = targets
        .iter()
        .filter(|target| target.target_kind.is_library())
        .collect::<Vec<_>>();
    if !libraries.is_empty() {
        output.push_str("## Library APIs\n\n");
    }
    for api in libraries {
        output.push_str(&format!(
            "- [`{}` v{}](/docs/api/{})\n",
            api.package.name,
            api.package.version,
            super::util::slug(&api.package.name)
        ));
    }
    output
}

pub(super) fn site_description(targets: &[ExtractedTarget]) -> String {
    let mut package_ids = HashSet::new();
    let unique_packages = targets
        .iter()
        .filter(|target| package_ids.insert(target.package.id.as_str()))
        .collect::<Vec<_>>();
    if unique_packages.len() == 1 {
        return unique_packages[0]
            .package
            .description
            .clone()
            .unwrap_or_else(|| {
                format!(
                    "Rust documentation for {}.",
                    unique_packages[0].package.name
                )
            });
    }
    format!(
        "Rust API and binary documentation for {} packages.",
        unique_packages.len()
    )
}

pub(super) fn default_title(targets: &[ExtractedTarget]) -> String {
    let mut package_ids = HashSet::new();
    let unique_packages = targets
        .iter()
        .filter(|target| package_ids.insert(target.package.id.as_str()))
        .collect::<Vec<_>>();
    if unique_packages.len() == 1 {
        unique_packages[0].package.name.clone()
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
    crate::formatting::json_string(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_site_paths_and_origins() {
        assert_eq!(normalize_base_path(" /docs/ ").unwrap(), "/docs");
        assert_eq!(normalize_base_path("/").unwrap(), "/");
        assert!(normalize_base_path("docs").is_err());
        assert_eq!(
            normalize_site_url("https://example.com/").unwrap(),
            "https://example.com"
        );
        assert!(normalize_site_url("https://example.com/docs").is_err());
    }

    #[test]
    fn recognizes_github_repository_urls() {
        assert_eq!(
            normalize_github_url("git+https://github.com/acme/demo.git").as_deref(),
            Some("https://github.com/acme/demo")
        );
        assert_eq!(normalize_github_url("https://gitlab.com/acme/demo"), None);
    }
}
