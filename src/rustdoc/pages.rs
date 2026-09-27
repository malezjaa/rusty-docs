use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use crate::metadata::Package;

use super::{ApiPage, DocLinkTarget, SignatureLinkTarget, members, signature, text};

type PendingPage<'a> = (String, String, String, Vec<String>, &'a Value, String);

pub(super) fn render_pages(
    json: &Value,
    package: &Package,
) -> Result<(Vec<ApiPage>, HashMap<String, SignatureLinkTarget>)> {
    let index = json
        .get("index")
        .and_then(Value::as_object)
        .context("rustdoc JSON has no item index")?;
    let paths = json
        .get("paths")
        .and_then(Value::as_object)
        .context("rustdoc JSON has no public item paths")?;
    let external_crates = json.get("external_crates").and_then(Value::as_object);
    let signature_targets = signature_link_targets(paths, external_crates);
    let crate_slug = text::slug(&package.name);
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
        pending.push((
            kind.to_owned(),
            name.to_owned(),
            rust_path,
            names,
            item,
            id.clone(),
        ));
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

    pending.sort_by(|left, right| left.2.cmp(&right.2).then(left.0.cmp(&right.0)));
    let mut reserved = HashSet::new();
    for (kind, name, rust_path, names, item, rustdoc_id) in pending {
        if !is_supported_kind(&kind) {
            continue;
        }

        let mut relative_path = if kind == "module" {
            let mut path = PathBuf::from("api").join(&crate_slug);
            for segment in names.iter().skip(1) {
                path.push(text::slug(segment));
            }
            path.join("index.mdx")
        } else {
            let mut path = PathBuf::from("api").join(&crate_slug);
            for segment in names.iter().skip(1).take(names.len().saturating_sub(2)) {
                path.push(text::slug(segment));
            }
            path.join(format!("{}.mdx", text::slug(&name)))
        };

        if !reserved.insert(relative_path.clone()) {
            let parent = relative_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            let filename = relative_path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("item")
                .to_owned();
            relative_path = parent.join(format!("{filename}--{kind}.mdx"));
            if !reserved.insert(relative_path.clone()) {
                relative_path = parent.join(format!("{filename}--{kind}-{}.mdx", item["id"]));
                reserved.insert(relative_path.clone());
            }
        }

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
        let api_toc = members::append_members(&mut markdown, &kind, item, index, &mut doc_links);
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
            },
        );
    }

    if pages.is_empty() {
        return Ok((Vec::new(), signature_targets));
    }

    Ok((pages.into_values().collect(), signature_targets))
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
            let Some((href, kind)) = external_rustdoc_url(target_id, paths, external_crates) else {
                continue;
            };
            *target = DocLinkTarget::Url { href, kind };
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
        "bool",
        "char",
        "f32",
        "f64",
        "i8",
        "i16",
        "i32",
        "i64",
        "i128",
        "isize",
        "str",
        "u8",
        "u16",
        "u32",
        "u64",
        "u128",
        "usize",
        "never",
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
    let root_url = if is_rustdoc_std_crate(crate_name) {
        "https://doc.rust-lang.org/nightly".to_owned()
    } else {
        format!("https://docs.rs/{crate_name}/latest")
    };
    let item_name = path.last()?;
    if kind == "module" {
        return Some(format!("{root_url}/{}/index.html", path.join("/")));
    }
    if kind == "primitive" {
        let crate_name = path.first()?;
        return Some(format!(
            "{root_url}/{crate_name}/primitive.{item_name}.html"
        ));
    }
    if kind == "keyword" {
        let crate_name = path.first()?;
        return Some(format!("{root_url}/{crate_name}/keyword.{item_name}.html"));
    }
    item_page_url(&root_url, path, kind, item_name)
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
) -> Option<(String, String)> {
    let item = paths.get(id)?;
    let crate_id = item.get("crate_id")?.as_u64()?;
    if crate_id == 0 {
        return None;
    }
    let external_crate = external_crates?.get(&crate_id.to_string())?;
    let crate_name = external_crate.get("name").and_then(Value::as_str)?;
    let root_url = if is_rustdoc_std_crate(crate_name) {
        "https://doc.rust-lang.org/nightly".to_owned()
    } else {
        let configured_root = external_crate
            .get("html_root_url")
            .and_then(Value::as_str)
            .map(|url| url.trim_end_matches('/'))
            .filter(|url| url.starts_with("https://") || url.starts_with("http://"));
        configured_root
            .map(str::to_owned)
            .unwrap_or_else(|| format!("https://docs.rs/{crate_name}/latest"))
    };
    let path = item.get("path")?.as_array()?;
    let segments = path.iter().map(Value::as_str).collect::<Option<Vec<_>>>()?;
    let kind = item.get("kind")?.as_str()?;
    let item_name = segments.last()?;

    if kind == "module" {
        return Some((
            format!("{root_url}/{}/index.html", segments.join("/")),
            kind.to_owned(),
        ));
    }
    if kind == "primitive" {
        let crate_name = segments.first()?;
        return Some((
            format!("{root_url}/{crate_name}/primitive.{item_name}.html"),
            kind.to_owned(),
        ));
    }
    if kind == "keyword" {
        let crate_name = segments.first()?;
        return Some((
            format!("{root_url}/{crate_name}/keyword.{item_name}.html"),
            kind.to_owned(),
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
            return Some((url, kind.to_owned()));
        }
    }

    Some((
        item_page_url(&root_url, &segments, kind, item_name)?,
        kind.to_owned(),
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
            pending.push((
                "reexport".to_owned(),
                name.to_owned(),
                path.join("::"),
                path,
                item,
                use_item
                    .get("id")
                    .and_then(Value::as_u64)
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| {
                        item.get("id")
                            .and_then(Value::as_u64)
                            .unwrap_or(0)
                            .to_string()
                    }),
            ));
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
