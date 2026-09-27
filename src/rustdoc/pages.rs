use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use crate::{config::ApiOptions, metadata::Package};

use super::{ApiPage, DocLinkTarget, SignatureLinkTarget, SourceSpan, members, signature, text};

struct PendingPage<'a> {
    kind: String,
    name: String,
    rust_path: String,
    segments: Vec<String>,
    item: &'a Value,
    rustdoc_id: String,
}

pub(super) fn render_pages(
    json: &Value,
    package: &Package,
    target_root: &Path,
    workspace_root: &Path,
    options: &ApiOptions,
) -> Result<(Vec<ApiPage>, HashMap<String, SignatureLinkTarget>)> {
    let index = json
        .get("index")
        .and_then(Value::as_object)
        .context("rustdoc JSON has no item index")?;
    let paths = json
        .get("paths")
        .and_then(Value::as_object)
        .context("rustdoc JSON has no item paths")?;
    let external_crates = json.get("external_crates").and_then(Value::as_object);
    let signature_targets = signature_link_targets(paths, external_crates);
    let root_id = json
        .get("root")
        .and_then(Value::as_u64)
        .map(|id| id.to_string());

    let mut pages = BTreeMap::<PathBuf, ApiPage>::new();
    let mut pending = Vec::new();
    for (id, path_info) in paths {
        if path_info.get("crate_id").and_then(Value::as_u64) != Some(0) {
            continue;
        }
        if root_id.as_deref() == Some(id.as_str()) {
            continue;
        }
        let Some(kind) = path_info.get("kind").and_then(Value::as_str) else {
            continue;
        };
        if kind == "variant" {
            continue;
        }
        let Some(path) = path_info.get("path").and_then(Value::as_array) else {
            continue;
        };
        let names = path
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if names.len() < 2 {
            continue;
        }
        let Some(item) = index.get(id) else {
            continue;
        };
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_else(|| names.last().map(String::as_str).unwrap_or("item"));
        let rust_path = names.join("::");
        pending.push(PendingPage {
            kind: kind.to_owned(),
            name: name.to_owned(),
            rust_path,
            segments: names,
            item,
            rustdoc_id: id.clone(),
        });
    }

    if let Some(root_id) = root_id.as_deref()
        && let Some(root) = index.get(root_id)
    {
        let root_name = root
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(&package.name)
            .to_owned();
        collect_reexports(root, &[root_name], index, &mut pending);
    }

    pending.sort_by(|left, right| {
        left.rust_path
            .cmp(&right.rust_path)
            .then(left.kind.cmp(&right.kind))
    });
    let mut reserved = HashSet::new();
    for pending_page in pending {
        let PendingPage {
            kind,
            name,
            rust_path,
            segments,
            item,
            rustdoc_id,
        } = pending_page;
        if !is_supported_kind(&kind) {
            continue;
        }
        if options.hides_kind(&kind) {
            continue;
        }

        let relative_path = if kind == "module" {
            let mut relative_path = target_root.to_path_buf();
            for segment in segments.iter().skip(1) {
                relative_path.push(text::slug(segment));
            }
            relative_path.join("index.mdx")
        } else {
            let mut relative_path = target_root.to_path_buf();
            for segment in segments
                .iter()
                .skip(1)
                .take(segments.len().saturating_sub(2))
            {
                relative_path.push(text::slug(segment));
            }
            relative_path.join(format!("{}.mdx", text::slug(&name)))
        };
        let relative_path = reserve_page_path(relative_path, &kind, item, &mut reserved);

        let title = name.clone();
        let docs = item.get("docs").and_then(Value::as_str).unwrap_or("");
        let description = text::first_sentence(docs)
            .unwrap_or_else(|| format!("Public {kind} from {}.", package.name));
        let signature = signature::signature(&kind, &name, item, index);
        let mut markdown = String::new();
        let mut doc_links = Vec::new();
        if kind != "module" {
            markdown.push_str(&format!(
                "<ApiSignature signature={{{}}} />\n\n",
                text::yaml_quote(&signature)
            ));
        }
        if !docs.trim().is_empty() {
            let rendered_docs = text::shift_doc_headings(docs.trim(), 1);
            doc_links.push(members::doc_link_group(&rendered_docs, item));
            markdown.push_str(&rendered_docs);
            markdown.push_str("\n\n");
        }
        let api_toc =
            members::append_members(&mut markdown, &kind, item, index, &mut doc_links, options);
        resolve_external_doc_links(&mut doc_links, paths, external_crates);
        let api_toc_json =
            serde_json::to_string(&api_toc).context("Could not serialize API TOC")?;
        markdown = format!(
            "---\ntitle: {}\ndescription: {}\napiToc: {}\n---\n\n{markdown}",
            text::yaml_quote(&title),
            text::yaml_quote(&description),
            api_toc_json
        );
        markdown = text::normalize_code_fences(&markdown);
        for group in &mut doc_links {
            group.markdown = text::normalize_code_fences(&group.markdown);
        }

        pages.insert(
            relative_path.clone(),
            ApiPage {
                relative_path,
                title,
                description,
                rust_path,
                rustdoc_id,
                doc_links,
                kind,
                markdown,
                source: source_span(item, package, workspace_root),
            },
        );
    }

    if pages.is_empty() {
        return Ok((Vec::new(), signature_targets));
    }

    Ok((pages.into_values().collect(), signature_targets))
}

fn source_span(item: &Value, package: &Package, workspace_root: &Path) -> Option<SourceSpan> {
    let span = item.get("span")?;
    let filename = PathBuf::from(span.get("filename")?.as_str()?);
    let path = if filename.is_absolute() {
        filename
            .strip_prefix(workspace_root)
            .or_else(|_| filename.strip_prefix(package.root()))
            .ok()?
            .to_path_buf()
    } else {
        package
            .root()
            .strip_prefix(workspace_root)
            .ok()
            .map(|package_path| package_path.join(&filename))
            .unwrap_or(filename)
    };
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return None;
    }
    let first_line = span.get("begin")?.as_array()?.first()?.as_u64()? as usize;
    let last_line = span.get("end")?.as_array()?.first()?.as_u64()? as usize;
    Some(SourceSpan {
        path,
        first_line,
        last_line,
    })
}

fn reserve_page_path(
    preferred: PathBuf,
    kind: &str,
    item: &Value,
    reserved: &mut HashSet<PathBuf>,
) -> PathBuf {
    if reserved.insert(preferred.clone()) {
        return preferred;
    }

    let parent = preferred.parent().unwrap_or_else(|| Path::new("."));
    let stem = preferred
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("item");
    let with_kind = parent.join(format!("{stem}--{kind}.mdx"));
    if reserved.insert(with_kind.clone()) {
        return with_kind;
    }

    let id = item.get("id").and_then(Value::as_u64).unwrap_or(0);
    let with_id = parent.join(format!("{stem}--{kind}-{id}.mdx"));
    reserved.insert(with_id.clone());
    with_id
}

fn resolve_external_doc_links(
    doc_links: &mut [super::DocLinkGroup],
    paths: &Map<String, Value>,
    external_crates: Option<&Map<String, Value>>,
) {
    for group in doc_links {
        for (_, target) in &mut group.links {
            let DocLinkTarget::Item(target_id) = target else {
                continue;
            };
            let Some((href, kind, crate_name, rust_path)) =
                external_rustdoc_url(target_id, paths, external_crates)
            else {
                continue;
            };
            *target = DocLinkTarget::Url {
                href,
                kind,
                crate_name,
                rust_path,
            };
        }
    }
}

fn signature_link_targets(
    paths: &Map<String, Value>,
    external_crates: Option<&Map<String, Value>>,
) -> HashMap<String, SignatureLinkTarget> {
    let mut targets = HashMap::new();
    for (id, item) in paths {
        let Some(crate_id) = item.get("crate_id").and_then(Value::as_u64) else {
            continue;
        };
        let path = item
            .get("path")
            .and_then(Value::as_array)
            .and_then(|segments| {
                segments
                    .iter()
                    .map(Value::as_str)
                    .collect::<Option<Vec<_>>>()
            });
        let kind = item.get("kind").and_then(Value::as_str).unwrap_or_default();
        if crate_id == 0 {
            targets.insert(
                id.clone(),
                SignatureLinkTarget {
                    local_id: Some(id.clone()),
                    crate_name: None,
                    rust_path: path.map(|path| path.join("::")),
                    kind: Some(kind.to_owned()),
                    external_url: None,
                },
            );
            continue;
        }

        let Some(path) = path else { continue };
        let Some(external_crate) =
            external_crates.and_then(|crates| crates.get(&crate_id.to_string()))
        else {
            continue;
        };
        let Some(crate_name) = external_crate.get("name").and_then(Value::as_str) else {
            continue;
        };
        let external_url = rustdoc_url(crate_name, &path, kind);
        targets.insert(
            id.clone(),
            SignatureLinkTarget {
                local_id: None,
                crate_name: Some(crate_name.to_owned()),
                rust_path: Some(path.join("::")),
                kind: Some(kind.to_owned()),
                external_url,
            },
        );
    }

    for primitive in [
        "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "str", "u8",
        "u16", "u32", "u64", "u128", "usize", "never",
    ] {
        targets.insert(
            format!("primitive:{primitive}"),
            SignatureLinkTarget {
                local_id: None,
                crate_name: None,
                rust_path: None,
                kind: Some("primitive".to_owned()),
                external_url: Some(format!(
                    "https://doc.rust-lang.org/nightly/core/primitive.{primitive}.html"
                )),
            },
        );
    }
    targets
}

fn rustdoc_url(crate_name: &str, path: &[&str], kind: &str) -> Option<String> {
    let root_url = rustdoc_root_url(crate_name, None);
    rustdoc_item_url(&root_url, path, kind)
}

fn rustdoc_root_url(crate_name: &str, configured_root: Option<&str>) -> String {
    if is_rustdoc_std_crate(crate_name) {
        return "https://doc.rust-lang.org/nightly".to_owned();
    }
    configured_root
        .map(|url| url.trim_end_matches('/'))
        .filter(|url| url.starts_with("https://") || url.starts_with("http://"))
        .map(str::to_owned)
        .unwrap_or_else(|| format!("https://docs.rs/{crate_name}/latest"))
}

fn rustdoc_item_url(root_url: &str, path: &[&str], kind: &str) -> Option<String> {
    let item_name = path.last()?;
    match kind {
        "module" => Some(format!("{root_url}/{}/index.html", path.join("/"))),
        "primitive" | "keyword" => {
            let crate_name = path.first()?;
            Some(format!("{root_url}/{crate_name}/{kind}.{item_name}.html"))
        }
        _ => item_page_url(root_url, path, kind, item_name),
    }
}

fn is_rustdoc_std_crate(crate_name: &str) -> bool {
    matches!(crate_name, "alloc" | "core" | "proc_macro" | "std" | "test")
}

fn is_supported_kind(kind: &str) -> bool {
    matches!(
        kind,
        "module"
            | "struct"
            | "enum"
            | "trait"
            | "trait_alias"
            | "type_alias"
            | "constant"
            | "static"
            | "union"
            | "function"
            | "macro"
            | "proc_macro"
            | "proc_attribute"
            | "proc_derive"
            | "reexport"
    )
}

fn external_rustdoc_url(
    id: &str,
    paths: &Map<String, Value>,
    external_crates: Option<&Map<String, Value>>,
) -> Option<(String, String, String, String)> {
    let item = paths.get(id)?;
    let crate_id = item.get("crate_id")?.as_u64()?;
    if crate_id == 0 {
        return None;
    }
    let external_crate = external_crates?.get(&crate_id.to_string())?;
    let crate_name = external_crate.get("name").and_then(Value::as_str)?;
    let root_url = rustdoc_root_url(
        crate_name,
        external_crate.get("html_root_url").and_then(Value::as_str),
    );
    let path = item.get("path")?.as_array()?;
    let segments = path.iter().map(Value::as_str).collect::<Option<Vec<_>>>()?;
    let kind = item.get("kind")?.as_str()?;
    let item_name = segments.last()?;
    let rust_path = segments.join("::");

    if matches!(kind, "module" | "primitive" | "keyword") {
        return Some((
            rustdoc_item_url(&root_url, &segments, kind)?,
            kind.to_owned(),
            crate_name.to_owned(),
            rust_path,
        ));
    }

    if segments.len() > 2 {
        let parent_path = &segments[..segments.len() - 1];
        if let Some(parent) = paths.values().find(|candidate| {
            candidate.get("crate_id").and_then(Value::as_u64) == Some(crate_id)
                && candidate
                    .get("path")
                    .and_then(Value::as_array)
                    .is_some_and(|candidate_path| {
                        candidate_path.len() == parent_path.len()
                            && candidate_path
                                .iter()
                                .zip(parent_path)
                                .all(|(left, right)| left.as_str() == Some(*right))
                    })
                && candidate
                    .get("kind")
                    .and_then(Value::as_str)
                    .is_some_and(is_item_page_kind)
        }) {
            let parent_kind = parent.get("kind")?.as_str()?;
            let parent_name = parent_path.last()?;
            let mut url = item_page_url(&root_url, parent_path, parent_kind, parent_name)?;
            if let Some(anchor_kind) = associated_item_anchor(kind) {
                url.push_str(&format!("#{anchor_kind}.{item_name}"));
            }
            return Some((url, kind.to_owned(), crate_name.to_owned(), rust_path));
        }
    }

    Some((
        rustdoc_item_url(&root_url, &segments, kind)?,
        kind.to_owned(),
        crate_name.to_owned(),
        rust_path,
    ))
}

fn associated_item_anchor(kind: &str) -> Option<&'static str> {
    match kind {
        "function" => Some("method"),
        "constant" => Some("associatedconstant"),
        "type_alias" => Some("associatedtype"),
        _ => None,
    }
}

fn is_item_page_kind(kind: &str) -> bool {
    matches!(
        kind,
        "struct" | "enum" | "union" | "trait" | "trait_alias" | "type_alias"
    )
}

fn item_page_url(root_url: &str, path: &[&str], kind: &str, name: &str) -> Option<String> {
    let crate_name = path.first()?;
    let filename_prefix = match kind {
        "struct" => "struct",
        "enum" => "enum",
        "union" => "union",
        "trait" => "trait",
        "trait_alias" => "traitalias",
        "type_alias" => "type",
        "function" => "fn",
        "constant" => "constant",
        "static" => "static",
        "macro" => "macro",
        "proc_macro" => "proc_macro",
        "proc_attribute" => "attr",
        "proc_derive" => "derive",
        "keyword" => "keyword",
        _ => return None,
    };
    let parent = if path.len() > 1 {
        path[..path.len() - 1].join("/")
    } else {
        crate_name.to_string()
    };
    Some(format!("{root_url}/{parent}/{filename_prefix}.{name}.html"))
}

fn collect_reexports<'a>(
    module: &'a Value,
    module_path: &[String],
    index: &'a serde_json::Map<String, Value>,
    pending: &mut Vec<PendingPage<'a>>,
) {
    let Some(items) = module
        .get("inner")
        .and_then(|inner| inner.get("module"))
        .and_then(|data| data.get("items"))
        .and_then(Value::as_array)
    else {
        return;
    };

    for id in items {
        let Some(item) = signature::get_item(index, id) else {
            continue;
        };
        if item.get("visibility").and_then(Value::as_str) != Some("public") {
            continue;
        }
        let Some(inner) = item.get("inner").and_then(Value::as_object) else {
            continue;
        };
        if let Some(use_item) = inner.get("use") {
            if use_item.get("is_glob").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let Some(name) = use_item.get("name").and_then(Value::as_str) else {
                continue;
            };
            let mut path = module_path.to_vec();
            path.push(name.to_owned());
            pending.push(PendingPage {
                kind: "reexport".to_owned(),
                name: name.to_owned(),
                rust_path: path.join("::"),
                segments: path,
                item,
                rustdoc_id: use_item
                    .get("id")
                    .and_then(Value::as_u64)
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| {
                        item.get("id")
                            .and_then(Value::as_u64)
                            .unwrap_or(0)
                            .to_string()
                    }),
            });
        } else if inner.contains_key("module") {
            let Some(name) = item.get("name").and_then(Value::as_str) else {
                continue;
            };
            let mut path = module_path.to_vec();
            path.push(name.to_owned());
            collect_reexports(item, &path, index, pending);
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn builds_urls_for_rustdoc_item_kinds() {
        let root = "https://docs.rs/demo/latest";

        assert_eq!(
            rustdoc_item_url(root, &["demo", "widget"], "module").as_deref(),
            Some("https://docs.rs/demo/latest/demo/widget/index.html")
        );
        assert_eq!(
            rustdoc_item_url(root, &["demo", "Widget"], "struct").as_deref(),
            Some("https://docs.rs/demo/latest/demo/struct.Widget.html")
        );
        assert_eq!(
            rustdoc_item_url(root, &["core", "u32"], "primitive").as_deref(),
            Some("https://docs.rs/demo/latest/core/primitive.u32.html")
        );
    }

    #[test]
    fn validates_configured_rustdoc_roots() {
        assert_eq!(
            rustdoc_root_url("demo", Some("https://example.com/docs/")),
            "https://example.com/docs"
        );
        assert_eq!(
            rustdoc_root_url("demo", Some("file:///tmp/docs")),
            "https://docs.rs/demo/latest"
        );
        assert_eq!(
            rustdoc_root_url("std", Some("https://example.com")),
            "https://doc.rust-lang.org/nightly"
        );
    }

    #[test]
    fn disambiguates_page_paths_by_kind_then_id() {
        let mut reserved = HashSet::new();
        let preferred = PathBuf::from("api/demo/item.mdx");

        assert_eq!(
            reserve_page_path(
                preferred.clone(),
                "struct",
                &json!({"id": 1}),
                &mut reserved
            ),
            preferred
        );
        assert_eq!(
            reserve_page_path(preferred.clone(), "enum", &json!({"id": 2}), &mut reserved),
            PathBuf::from("api/demo/item--enum.mdx")
        );
        assert_eq!(
            reserve_page_path(preferred, "enum", &json!({"id": 3}), &mut reserved),
            PathBuf::from("api/demo/item--enum-3.mdx")
        );
    }

    #[test]
    fn makes_relative_spans_workspace_relative() {
        let package = Package {
            id: "demo".to_owned(),
            name: "demo".to_owned(),
            version: "1.0.0".to_owned(),
            manifest_path: PathBuf::from("/work/crates/demo/Cargo.toml"),
            description: None,
            repository: None,
            targets: Vec::new(),
        };
        let item = json!({
            "span": {
                "filename": "src/lib.rs",
                "begin": [10, 1],
                "end": [12, 2]
            }
        });

        let span = source_span(&item, &package, Path::new("/work")).unwrap();
        assert_eq!(span.path, PathBuf::from("crates/demo/src/lib.rs"));
        assert_eq!(span.first_line, 10);
        assert_eq!(span.last_line, 12);
    }
}
