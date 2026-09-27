mod members;
mod pages;
mod signature;
mod text;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::{metadata::Package, terminal::Terminal};

#[derive(Clone, Debug)]
pub struct ApiPage {
    pub relative_path: PathBuf,
    pub title: String,
    pub description: String,
    pub rust_path: String,
    pub rustdoc_id: String,
    pub doc_links: Vec<DocLinkGroup>,
    pub kind: String,
    pub markdown: String,
}

#[derive(Clone, Debug)]
pub struct DocLinkGroup {
    pub markdown: String,
    pub links: Vec<(String, DocLinkTarget)>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTocItem {
    pub title: String,
    pub url: String,
    pub depth: usize,
    pub insert_at: usize,
    pub shift_next: usize,
    pub shift_depth: usize,
}

#[derive(Clone, Debug)]
pub enum DocLinkTarget {
    Item(String),
    Url { href: String, kind: String },
}

pub(crate) fn member_anchor(kind: &str, name: &str, id: u64) -> String {
    let mut slug = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug.push_str("item");
    }
    format!("api-{kind}-{slug}-{id}")
}

#[derive(Debug)]
pub struct ExtractedApi {
    pub package: Package,
    pub crate_name: String,
    pub pages: Vec<ApiPage>,
    pub signature_targets: HashMap<String, SignatureLinkTarget>,
}

#[derive(Clone, Debug)]
pub struct SignatureLinkTarget {
    pub local_id: Option<String>,
    pub crate_name: Option<String>,
    pub rust_path: Option<String>,
    pub kind: Option<String>,
    pub external_url: Option<String>,
}

pub struct ExtractOptions<'a> {
    pub target_name: &'a str,
    pub target_directory: &'a Path,
    pub toolchain: &'a str,
    pub features: &'a [String],
    pub all_features: bool,
    pub no_default_features: bool,
}

pub fn extract(
    package: &Package,
    options: ExtractOptions<'_>,
    terminal: &Terminal,
) -> Result<ExtractedApi> {
    let ExtractOptions {
        target_name,
        target_directory,
        toolchain,
        features,
        all_features,
        no_default_features,
    } = options;
    let stage = terminal.stage(format!("Extracting {} API", package.name));
    let mut command = Command::new("cargo");
    command
        .arg(format!("+{toolchain}"))
        .args(["rustdoc", "--manifest-path"])
        .arg(&package.manifest_path)
        .args(["--package", &package.name, "--lib"]);
    if no_default_features {
        command.arg("--no-default-features");
    }
    if all_features {
        command.arg("--all-features");
    } else if !features.is_empty() {
        command.arg("--features").arg(features.join(","));
    }
    command.args(["--", "-Z", "unstable-options", "--output-format", "json"]);
    if terminal.is_color_enabled() {
        if std::env::var_os("FORCE_COLOR").is_none() {
            command.env("FORCE_COLOR", "1");
        }
        if std::env::var_os("CARGO_TERM_COLOR").is_none() {
            command.env("CARGO_TERM_COLOR", "always");
        }
    }

    let output = command.output().with_context(|| {
        format!("Could not run cargo +{toolchain} rustdoc. Install that Rust toolchain first.")
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let details = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        bail!(
            "rustdoc failed for package '{}'. Rustdoc JSON needs a nightly toolchain.\n{}",
            package.name,
            details
        );
    }
    if terminal.is_verbose() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.trim().is_empty() {
            print!("{stdout}");
        }
        if !stderr.trim().is_empty() {
            eprint!("{stderr}");
        }
    }

    let crate_name = target_name.replace('-', "_");
    let json_path = target_directory
        .join("doc")
        .join(format!("{crate_name}.json"));
    let json_data = fs::read(&json_path).with_context(|| {
        format!(
            "Cargo completed rustdoc, but its JSON output was not found at {}",
            json_path.display()
        )
    })?;
    let json: Value = serde_json::from_slice(&json_data)
        .with_context(|| format!("Could not parse rustdoc JSON at {}", json_path.display()))?;
    let (pages, signature_targets) = pages::render_pages(&json, package)?;
    let page_count = terminal.highlight(&pages.len().to_string());
    let package_name = terminal.highlight(&package.name);
    stage.finish(&format!(
        "Extracted {page_count} API pages from {package_name}"
    ));

    Ok(ExtractedApi {
        package: package.clone(),
        crate_name,
        pages,
        signature_targets,
    })
}
