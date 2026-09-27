use std::path::Path;

use super::metadata;

pub(super) fn mdx_string(value: &str) -> String {
    format!("{{{}}}", metadata::yaml_quote(value))
}

pub(super) fn page_route(path: &Path) -> String {
    let mut parts = path
        .components()
        .filter_map(|part| part.as_os_str().to_str())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if let Some(last) = parts.last_mut() {
        if last == "index.md" || last == "index.mdx" {
            parts.pop();
        } else if let Some(stem) = last
            .strip_suffix(".mdx")
            .or_else(|| last.strip_suffix(".md"))
        {
            *last = stem.to_owned();
        }
    }
    format!("/docs/{}", parts.join("/"))
}

pub(super) fn slug(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "package".to_owned()
    } else {
        slug
    }
}
