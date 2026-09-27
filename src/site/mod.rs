mod api_pages;
mod content;
mod metadata;
mod npm;
mod template;
mod util;

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::Serialize;

use crate::{
    config::Config, metadata::CargoMetadata, rustdoc::ExtractedTarget, terminal::Terminal,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteConfig {
    title: String,
    description: String,
    base_path: String,
    content_position: String,
    site_url: Option<String>,
    repository_url: Option<String>,
    packages: Vec<PackageSummary>,
    api_items: Vec<ApiItemSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageSummary {
    name: String,
    crate_name: String,
    version: String,
    description: String,
    api_items: usize,
    slug: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiItemSummary {
    kind: String,
    route: String,
}

pub struct BuildResult {
    pub site_dir: PathBuf,
    pub static_dir: PathBuf,
}

pub fn generate_and_build(
    config: &Config,
    metadata: &CargoMetadata,
    targets: &[ExtractedTarget],
    content_override: Option<&Path>,
    output_override: Option<&Path>,
    terminal: &Terminal,
) -> Result<BuildResult> {
    let workspace_root = &metadata.workspace_root;
    let content_dir = content_override
        .map(Path::to_path_buf)
        .unwrap_or_else(|| config.site.content_dir.clone());
    let content_dir = metadata::resolve_from(workspace_root, &content_dir);
    let output_dir = output_override
        .map(Path::to_path_buf)
        .unwrap_or_else(|| config.site.output_dir.clone());
    let site_dir = metadata::resolve_from(workspace_root, &output_dir);
    let content_target = site_dir.join("content/docs");
    if content_dir == content_target || content_dir.starts_with(&content_target) {
        bail!("content_dir cannot point inside the generated site's content/docs folder");
    }

    template::ensure_template(&site_dir)?;
    let site_config = create_site_config(config, targets)?;

    let stage = terminal.stage("Writing site metadata and syncing content");
    metadata::write_json(&site_dir.join("rusty-docs-site.json"), &site_config)?;
    content::sync_content(&content_dir, &content_target)?;
    metadata::write_markdown(
        &content_target.join("index.mdx"),
        &metadata::docs_index(&site_config.title, &site_config.description, targets),
    )?;
    if targets.iter().any(|target| target.target_kind.is_library()) {
        metadata::write_markdown(
            &content_target.join("api/index.md"),
            &metadata::api_index(targets),
        )?;
    }
    stage.finish("Wrote site metadata and synced content");

    let api_page_count = targets
        .iter()
        .map(|target| target.pages.len() + 1)
        .sum::<usize>();
    let workspace_routes = api_pages::WorkspaceRoutes::from_targets(targets);
    let progress = terminal.progress(api_page_count as u64, "Generating reference pages");
    for target in targets {
        api_pages::write_target_pages(
            &content_target,
            target,
            &workspace_routes,
            progress.as_ref(),
        )?;
    }
    if let Some(progress) = progress {
        let page_count = terminal.highlight(&api_page_count.to_string());
        terminal.finish_progress(progress, &format!("Generated {page_count} reference pages"));
    }

    npm::check_node_tools()?;
    npm::install_dependencies_if_needed(&site_dir, terminal)?;
    let build_stage = terminal.stage("Building static Next.js site");
    npm::run_npm(&site_dir, &["run", "build"], terminal)?;
    let static_dir = site_dir.join("out");
    if !static_dir.join("index.html").is_file() {
        bail!(
            "Next.js completed, but the static site was not found at {}",
            static_dir.display()
        );
    }
    build_stage.finish("Static site ready");

    Ok(BuildResult {
        site_dir,
        static_dir,
    })
}

fn create_site_config(config: &Config, targets: &[ExtractedTarget]) -> Result<SiteConfig> {
    let base_path = metadata::normalize_base_path(&config.site.base_path)?;
    let site_url = config
        .site
        .url
        .as_deref()
        .map(metadata::normalize_site_url)
        .transpose()?;
    let title = config
        .site
        .title
        .clone()
        .unwrap_or_else(|| metadata::default_title(targets));
    let description = metadata::site_description(targets);
    let repository_url = targets
        .iter()
        .filter_map(|target| target.package.repository.as_deref())
        .find_map(metadata::normalize_github_url);
    let package_summaries = targets
        .iter()
        .filter(|target| target.target_kind.is_library())
        .map(|api| PackageSummary {
            name: api.package.name.clone(),
            crate_name: api.crate_name.clone(),
            version: api.package.version.clone(),
            description: api
                .package
                .description
                .clone()
                .unwrap_or_else(|| format!("API reference for {}", api.package.name)),
            api_items: api
                .pages
                .iter()
                .filter(|page| page.kind != "module")
                .count(),
            slug: util::slug(&api.package.name),
        })
        .collect::<Vec<_>>();
    let api_items = targets
        .iter()
        .flat_map(|target| {
            let mut items = Vec::with_capacity(target.pages.len() + 1);
            if target.target_kind.is_binary() {
                items.push(ApiItemSummary {
                    kind: "binary".to_owned(),
                    route: util::page_route(&target.route_root.join("index.mdx")),
                });
            }
            items.extend(target.pages.iter().map(|page| ApiItemSummary {
                kind: page.kind.clone(),
                route: util::page_route(&page.relative_path),
            }));
            items
        })
        .collect();
    Ok(SiteConfig {
        title,
        description,
        base_path,
        content_position: config.site.content_position.as_str().to_owned(),
        site_url,
        repository_url,
        packages: package_summaries,
        api_items,
    })
}
