use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use indicatif::ProgressBar;

use crate::rustdoc::{ExtractedApi, member_anchor};

use super::{metadata, util};

pub(super) fn write_package_pages(
    content_root: &Path,
    api: &ExtractedApi,
    workspace_routes: &HashMap<(String, String), String>,
    progress: Option<&ProgressBar>,
) -> Result<()> {
    let crate_slug = util::slug(&api.package.name);
    let package_root = PathBuf::from("api").join(&crate_slug);
    let description = api
        .package
        .description
        .as_deref()
        .unwrap_or("Public Rust API reference.");
    let title = format!("{} v{}", api.package.name, api.package.version);
    let mut route_by_id = api
        .pages
        .iter()
        .filter(|page| page.kind != "reexport")
        .map(|page| {
            (
                page.rustdoc_id.clone(),
                util::page_route(&page.relative_path),
            )
        })
        .collect::<HashMap<_, _>>();
    add_local_member_routes(api, &mut route_by_id);
    let api_items = grouped_api_items(&api.pages, 3);
    let overview = format!(
        "---\ntitle: {}\ndescription: {}\n---\n\n# `{}`\n\n{}\n\nThis reference contains {} public API pages.\n\n## Public items\n\n{}",
        metadata::yaml_quote(&title),
        metadata::yaml_quote(description),
        api.package.name,
        description,
        api.pages.len(),
        api_items
    );
    metadata::write_markdown(
        &content_root.join(&package_root).join("index.mdx"),
        &add_api_component_imports(&overview),
    )?;

    for page in &api.pages {
        let path = content_root.join(&page.relative_path);
        if page.relative_path == package_root.join("index.mdx") {
            continue;
        }
        let mut markdown = rewrite_doc_link_groups(&page.markdown, &page.doc_links, &route_by_id);
        if page.kind == "module" {
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
            if !items.is_empty() {
                markdown.push_str("\n## Public items\n\n");
                markdown.push_str(&items);
            }
        }
        rewrite_signature_type_links(&mut markdown, api, &route_by_id, workspace_routes);
        metadata::write_markdown(&path, &add_api_component_imports(&markdown))?;
        if let Some(progress) = progress {
            progress.inc(1);
        }
    }
    Ok(())
}

fn add_local_member_routes(api: &ExtractedApi, route_by_id: &mut HashMap<String, String>) {
    for (id, target) in &api.signature_targets {
        if target.local_id.as_deref() != Some(id.as_str()) || route_by_id.contains_key(id) {
            continue;
        }
        let (Some(rust_path), Some(kind)) = (target.rust_path.as_deref(), target.kind.as_deref())
        else {
            continue;
        };
        let Some((parent_path, name)) = rust_path.rsplit_once("::") else {
            continue;
        };
        let Some(parent) = api.pages.iter().find(|page| page.rust_path == parent_path) else {
            continue;
        };
        let route = util::page_route(&parent.relative_path);
        let fragment = match (parent.kind.as_str(), kind) {
            ("trait", "function" | "type_alias" | "constant") => util::slug(name),
            ("enum", "variant") => util::slug(name),
            ("struct" | "enum" | "union", "function" | "type_alias" | "constant" | "static") => {
                let Ok(id) = id.parse::<u64>() else {
                    continue;
                };
                member_anchor("member", name, id)
            }
            _ => continue,
        };
        route_by_id.insert(id.clone(), format!("{route}#{fragment}"));
    }
}

fn rewrite_signature_type_links(
    markdown: &mut String,
    api: &ExtractedApi,
    route_by_id: &HashMap<String, String>,
    workspace_routes: &HashMap<(String, String), String>,
) {
    const LINK_START: char = '\u{e000}';
    const LINK_SEPARATOR: char = '\u{e001}';
    const LINK_END: char = '\u{e002}';

    let source = markdown.as_str();
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative_start) = source[cursor..].find(LINK_START) {
        let start = cursor + relative_start;
        output.push_str(&source[cursor..start]);
        let key_start = start + LINK_START.len_utf8();
        let Some(relative_separator) = source[key_start..].find(LINK_SEPARATOR) else {
            output.push(LINK_START);
            cursor = key_start;
            continue;
        };
        let separator = key_start + relative_separator;
        let label_start = separator + LINK_SEPARATOR.len_utf8();
        let Some(relative_end) = source[label_start..].find(LINK_END) else {
            output.push(LINK_START);
            cursor = key_start;
            continue;
        };
        let end = label_start + relative_end;
        let key = &source[key_start..separator];
        let label = &source[label_start..end];
        let target = api.signature_targets.get(key).and_then(|target| {
            target
                .local_id
                .as_ref()
                .and_then(|id| route_by_id.get(id))
                .map(|route| ("L", route.as_str()))
                .or_else(|| {
                    target
                        .crate_name
                        .as_ref()
                        .zip(target.rust_path.as_ref())
                        .and_then(|(crate_name, path)| {
                            workspace_routes.get(&(crate_name.clone(), path.clone()))
                        })
                        .map(|route| ("L", route.as_str()))
                })
                .or_else(|| target.external_url.as_deref().map(|url| ("E", url)))
        });
        if let Some((kind, href)) = target {
            output.push(LINK_START);
            output.push_str(kind);
            output.push(':');
            output.push_str(href);
            output.push(LINK_SEPARATOR);
        }
        output.push_str(label);
        if target.is_some() {
            output.push(LINK_END);
        }
        cursor = end + LINK_END.len_utf8();
    }
    output.push_str(&source[cursor..]);
    *markdown = output;
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

fn grouped_api_items<'a>(
    pages: impl IntoIterator<Item = &'a crate::rustdoc::ApiPage>,
    level: usize,
) -> String {
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
                        page.description.as_str()
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

fn rewrite_doc_link_groups(
    markdown: &str,
    groups: &[crate::rustdoc::DocLinkGroup],
    route_by_id: &HashMap<String, String>,
) -> String {
    let source = normalize_reference_link_destinations(markdown);
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    for group in groups {
        let chunk = normalize_reference_link_destinations(&group.markdown);
        if chunk.is_empty() {
            continue;
        }
        let Some(relative_start) = source[cursor..].find(&chunk) else {
            continue;
        };
        let start = cursor + relative_start;
        let end = start + chunk.len();
        output.push_str(&source[cursor..start]);
        let mut rewritten = chunk;
        rewrite_link_tokens(&mut rewritten, &group.links, route_by_id);
        output.push_str(&rewritten);
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    output
}

fn rewrite_link_tokens(
    output: &mut String,
    links: &[(String, crate::rustdoc::DocLinkTarget)],
    route_by_id: &HashMap<String, String>,
) {
    for (label, target) in links {
        let (route, external_kind) = match target {
            crate::rustdoc::DocLinkTarget::Item(target_id) => {
                let Some(route) = route_by_id.get(target_id) else {
                    continue;
                };
                (route.as_str(), None)
            }
            crate::rustdoc::DocLinkTarget::Url { href, kind } => {
                (href.as_str(), Some(kind.as_str()))
            }
        };
        let href = html_escape_attribute(route);
        let labels = if label.starts_with('`') && label.ends_with('`') {
            vec![(format!("[{label}]"), true)]
        } else {
            vec![
                (format!("[{label}]"), false),
                (format!("[`{label}`]"), true),
            ]
        };
        for (token, code_label) in labels {
            let replacement = if let Some(kind) = external_kind {
                let display_label = if code_label {
                    format!("<code>{}</code>", html_escape_text(label.trim_matches('`')))
                } else {
                    html_escape_text(label)
                };
                format!(
                    "<a href=\"{href}\" data-kind=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">{display_label}</a>",
                    html_escape_attribute(kind)
                )
            } else {
                format!("{token}({route})")
            };
            *output = replace_shortcut_link(output, &token, &replacement);
        }
    }
}

fn replace_shortcut_link(markdown: &str, token: &str, replacement: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut cursor = 0;
    while let Some(relative_start) = markdown[cursor..].find(token) {
        let start = cursor + relative_start;
        let end = start + token.len();
        let previous = markdown[..start].chars().next_back();
        let next = markdown[end..].chars().next();
        let explicit_link_end =
            (previous != Some('!') && next == Some('(') && !inside_api_signature(markdown, start))
                .then(|| markdown_link_end(markdown, end))
                .flatten();
        if let Some(link_end) = explicit_link_end {
            output.push_str(&markdown[cursor..start]);
            output.push_str(replacement);
            cursor = link_end + 1;
            continue;
        }
        let is_shortcut_link = previous != Some('!')
            && !matches!(next, Some('(' | '[' | ':'))
            && !inside_api_signature(markdown, start);
        output.push_str(&markdown[cursor..start]);
        if is_shortcut_link {
            output.push_str(replacement);
        } else {
            output.push_str(token);
        }
        cursor = end;
    }
    output.push_str(&markdown[cursor..]);
    output
}

fn markdown_link_end(markdown: &str, open_paren: usize) -> Option<usize> {
    let mut depth = 0;
    let mut escaped = false;
    for (offset, character) in markdown[open_paren..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_paren + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn inside_api_signature(markdown: &str, index: usize) -> bool {
    let Some(start) = markdown[..index].rfind("<ApiSignature") else {
        return false;
    };
    markdown[start..]
        .find("/>")
        .is_some_and(|relative_end| start + relative_end > index)
}

fn normalize_reference_link_destinations(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut in_fenced_code = false;
    for line in markdown.split_inclusive('\n') {
        let trimmed_line = line.trim_start();
        if trimmed_line.starts_with("```") || trimmed_line.starts_with("~~~") {
            in_fenced_code = !in_fenced_code;
            output.push_str(line);
            continue;
        }
        if in_fenced_code {
            output.push_str(line);
            continue;
        }
        let line_without_ending = line.strip_suffix('\n').unwrap_or(line);
        let leading_space = line_without_ending.len() - line_without_ending.trim_start().len();
        let definition = &line_without_ending[leading_space..];
        let Some(label_end) = definition.find("]:") else {
            output.push_str(line);
            continue;
        };
        let destination_start = label_end + 2;
        let destination_with_space = &definition[destination_start..];
        let whitespace = destination_with_space.len() - destination_with_space.trim_start().len();
        let destination = &destination_with_space[whitespace..];
        let Some(link_text_end) = destination
            .strip_prefix('[')
            .and_then(|value| value.find(']'))
        else {
            output.push_str(line);
            continue;
        };
        let after_link_text = link_text_end + 2;
        let Some(after_open_paren) = destination[after_link_text..].strip_prefix('(') else {
            output.push_str(line);
            continue;
        };
        let Some(href_end) = after_open_paren.find(')') else {
            output.push_str(line);
            continue;
        };
        let href = &after_open_paren[..href_end];
        if !(href.starts_with("https://") || href.starts_with("http://")) {
            output.push_str(line);
            continue;
        }

        let destination_offset = leading_space + destination_start + whitespace;
        let end_offset = destination_offset + after_link_text + 1 + href_end + 1;
        output.push_str(&line_without_ending[..destination_offset]);
        output.push('<');
        output.push_str(href);
        output.push('>');
        output.push_str(&line_without_ending[end_offset..]);
        if line.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}

fn html_escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_escape_attribute(value: &str) -> String {
    html_escape_text(value).replace('"', "&quot;")
}
