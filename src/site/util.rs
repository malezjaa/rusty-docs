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
    crate::formatting::slug(value, "package")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn converts_markdown_paths_to_routes() {
        assert_eq!(
            page_route(Path::new("api/demo/index.mdx")),
            "/docs/api/demo"
        );
        assert_eq!(
            page_route(Path::new("api/demo/widget.md")),
            "/docs/api/demo/widget"
        );
    }
}
