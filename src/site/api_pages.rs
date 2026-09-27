mod links;

use std::{collections::HashMap, path::Path};

use anyhow::Result;
use indicatif::ProgressBar;

use crate::rustdoc::{ApiPage, ExtractedTarget, SignatureLinkTarget, member_anchor};

use super::{metadata, util};

type ItemRoutes = HashMap<String, String>;

#[derive(Default)]
pub(super) struct WorkspaceRoutes {
    by_crate: HashMap<String, HashMap<String, String>>,
}

struct TargetOverview {
    title: String,
    description: String,
    heading: String,
    item_heading: &'static str,
    item_scope: &'static str,
}

impl TargetOverview {
    fn new(target: &ExtractedTarget) -> Self {
        if target.target_kind.is_library() {
            Self {
                title: format!("{} v{}", target.package.name, target.package.version),
                description: target
                    .package
                    .description
                    .clone()
                    .unwrap_or_else(|| "Public Rust API reference.".to_owned()),
                heading: format!("# `{}`", target.package.name),
                item_heading: "Public items",
                item_scope: "public API",
            }
        } else {
            Self {
                title: format!("{} binary", target.target_name),
                description: format!(
                    "Source reference for the `{}` binary in package `{}`.",
                    target.target_name, target.package.name
                ),
                heading: format!("# `{}`", target.target_name),
                item_heading: "Items",
                item_scope: "documented item",
            }
        }
    }

    fn render(&self, page_count: usize, api_items: &str) -> String {
        format!(
            "---\ntitle: {}\ndescription: {}\n---\n\n{}\n\n{}\n\nThis reference contains {} {} pages.\n\n## {}\n\n{}",
            metadata::yaml_quote(&self.title),
            metadata::yaml_quote(&self.description),
            self.heading,
            self.description,
            page_count,
            self.item_scope,
            self.item_heading,
            api_items,
        )
    }
}

impl WorkspaceRoutes {
    pub(super) fn from_targets(targets: &[ExtractedTarget]) -> Self {
        let mut routes = Self::default();
        for target in targets {
            routes.add_target(target);
        }
        routes
    }

    fn insert(&mut self, crate_name: &str, rust_path: &str, route: String) {
        self.by_crate
            .entry(crate_name.to_owned())
            .or_default()
            .insert(rust_path.to_owned(), route);
    }

    fn get(&self, crate_name: &str, rust_path: &str) -> Option<&str> {
        self.by_crate
            .get(crate_name)?
            .get(rust_path)
            .map(String::as_str)
    }

    fn add_target(&mut self, api: &ExtractedTarget) {
        if !api.target_kind.is_library() {
            return;
        }

        let package_root = api.route_root.join("index.mdx");
        self.insert(
            &api.crate_name,
            &api.crate_name,
            util::page_route(&package_root),
        );

        for page in &api.pages {
            self.insert(
                &api.crate_name,
                &page.rust_path,
                util::page_route(&page.relative_path),
            );
        }

        for (id, target) in &api.signature_targets {
            if target.local_id.as_deref() != Some(id.as_str()) {
                continue;
            }
            let Some(rust_path) = target.rust_path.as_deref() else {
                continue;
            };
            if let Some(route) = local_member_route(api, id, target) {
                self.insert(&api.crate_name, rust_path, route);
            }
        }
    }
}

pub(super) fn write_target_pages(
    content_root: &Path,
    api: &ExtractedTarget,
    workspace_routes: &WorkspaceRoutes,
    progress: Option<&ProgressBar>,
) -> Result<()> {
    let target_root = &api.route_root;
    let overview = TargetOverview::new(api);
    let route_by_id = item_routes(api);
    let api_items = grouped_api_items(&api.pages, 3);
    let overview_markdown = overview.render(api.pages.len(), &api_items);
    metadata::write_markdown(
        &content_root.join(target_root).join("index.mdx"),
        &add_api_component_imports(&overview_markdown),
    )?;
    if let Some(progress) = progress {
        progress.inc(1);
    }

    for page in &api.pages {
        let path = content_root.join(&page.relative_path);
        if page.relative_path == target_root.join("index.mdx") {
            continue;
        }
        let mut markdown = links::rewrite_doc_link_groups(
            &page.markdown,
            &page.doc_links,
            &route_by_id,
            workspace_routes,
        );
        append_module_items(&mut markdown, page, api);
        links::rewrite_signature_type_links(&mut markdown, api, &route_by_id, workspace_routes);
        metadata::write_markdown(&path, &add_api_component_imports(&markdown))?;
        if let Some(progress) = progress {
            progress.inc(1);
        }
    }
    Ok(())
}

fn item_routes(api: &ExtractedTarget) -> ItemRoutes {
    let mut routes = api
        .pages
        .iter()
        .filter(|page| page.kind != "reexport")
        .map(|page| {
            (
                page.rustdoc_id.clone(),
                util::page_route(&page.relative_path),
            )
        })
        .collect();
    add_local_member_routes(api, &mut routes);
    routes
}

fn append_module_items(markdown: &mut String, page: &ApiPage, api: &ExtractedTarget) {
    if page.kind != "module" {
        return;
    }

    let module_dir = page.relative_path.parent().unwrap_or(Path::new("."));
    let children = api.pages.iter().filter(|candidate| {
        let Some(parent) = candidate.relative_path.parent() else {
            return false;
        };
        let parent_module = if candidate.kind == "module" {
            parent.parent().unwrap_or(parent)
        } else {
            parent
        };
        parent_module == module_dir
    });
    let items = grouped_api_items(children, 3);
    if items.is_empty() {
        return;
    }

    let heading = if api.target_kind.is_library() {
        "Public items"
    } else {
        "Items"
    };
    markdown.push_str(&format!("\n## {heading}\n\n{items}"));
}

fn add_local_member_routes(api: &ExtractedTarget, route_by_id: &mut ItemRoutes) {
    for (id, target) in &api.signature_targets {
        if target.local_id.as_deref() != Some(id.as_str()) || route_by_id.contains_key(id) {
            continue;
        }
        if let Some(route) = local_member_route(api, id, target) {
            route_by_id.insert(id.clone(), route);
        }
    }
}

fn local_member_route(
    api: &ExtractedTarget,
    id: &str,
    target: &SignatureLinkTarget,
) -> Option<String> {
    let (rust_path, kind) = (target.rust_path.as_deref()?, target.kind.as_deref()?);
    let (parent_path, name) = rust_path.rsplit_once("::")?;
    let parent = api
        .pages
        .iter()
        .find(|page| page.rust_path == parent_path)?;
    let route = util::page_route(&parent.relative_path);
    let fragment = match (parent.kind.as_str(), kind) {
        ("trait", "function" | "type_alias" | "constant") => util::slug(name),
        ("enum", "variant") => util::slug(name),
        ("struct" | "enum" | "union", "function" | "type_alias" | "constant" | "static") => {
            let id = id.parse::<u64>().ok()?;
            member_anchor("member", name, id)
        }
        _ => return None,
    };
    Some(format!("{route}#{fragment}"))
}

const API_KIND_SECTIONS: &[(&str, &str)] = &[
    ("module", "Modules"),
    ("struct", "Structs"),
    ("enum", "Enums"),
    ("union", "Unions"),
    ("constant", "Constants"),
    ("static", "Statics"),
    ("function", "Functions"),
    ("type_alias", "Type Aliases"),
    ("trait", "Traits"),
    ("trait_alias", "Trait Aliases"),
    ("macro", "Macros"),
    ("proc_macro", "Procedural Macros"),
    ("proc_attribute", "Attribute Macros"),
    ("proc_derive", "Derive Macros"),
    ("reexport", "Re-exports"),
];

fn grouped_api_items<'a>(pages: impl IntoIterator<Item = &'a ApiPage>, level: usize) -> String {
    let pages = pages.into_iter().collect::<Vec<_>>();
    API_KIND_SECTIONS
        .iter()
        .filter_map(|(kind, heading)| {
            let mut items = pages
                .iter()
                .copied()
                .filter(|page| page.kind == *kind)
                .collect::<Vec<_>>();
            if items.is_empty() {
                return None;
            }
            items.sort_by(|left, right| {
                left.title
                    .to_lowercase()
                    .cmp(&right.title.to_lowercase())
                    .then(left.rust_path.cmp(&right.rust_path))
            });
            let heading_marks = "#".repeat(level);
            let list = items
                .iter()
                .map(|page| {
                    format!(
                        "- <ApiItemLink href={} kind={} name={} rustPath={} />\n  {}",
                        util::mdx_string(&util::page_route(&page.relative_path)),
                        util::mdx_string(&page.kind),
                        util::mdx_string(&page.title),
                        util::mdx_string(&page.rust_path),
                        links::mdx_text(&page.description)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            Some(format!("{heading_marks} {heading}\n\n{list}"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn add_api_component_imports(markdown: &str) -> String {
    let mut components = Vec::new();
    if markdown.contains("<ApiItemLink") {
        components.push("ApiItemLink");
    }
    if markdown.contains("<ApiItemMark") {
        components.push("ApiItemMark");
    }
    if markdown.contains("<ApiSignature") {
        components.push("ApiSignature");
    }
    if markdown.contains("<ApiMember") {
        components.push("ApiMember");
    }
    if markdown.contains("<ApiImpl") {
        components.push("ApiImpl");
    }
    if components.is_empty() {
        return markdown.to_owned();
    }

    let import = format!(
        "import {{ {} }} from '@/components/api-item';\n\n",
        components.join(", ")
    );
    let Some(frontmatter_body) = markdown.strip_prefix("---\n") else {
        return format!("{import}{markdown}");
    };
    let Some(frontmatter_end) = frontmatter_body.find("\n---\n") else {
        return format!("{import}{markdown}");
    };
    let insertion = 4 + frontmatter_end + 5;
    format!(
        "{}{import}{}",
        &markdown[..insertion],
        &markdown[insertion..]
    )
}
