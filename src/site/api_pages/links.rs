use crate::rustdoc::{
    DocLinkGroup, DocLinkTarget, ExtractedTarget, SIGNATURE_LINK_END, SIGNATURE_LINK_SEPARATOR,
    SIGNATURE_LINK_START, SignatureLinkTarget,
};

use super::{ItemRoutes, WorkspaceRoutes};

pub(super) fn rewrite_signature_type_links(
    markdown: &mut String,
    api: &ExtractedTarget,
    route_by_id: &ItemRoutes,
    workspace_routes: &WorkspaceRoutes,
) {
    let (frontmatter, source) = split_frontmatter(markdown);
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative_start) = source[cursor..].find(SIGNATURE_LINK_START) {
        let start = cursor + relative_start;
        output.push_str(&source[cursor..start]);
        let key_start = start + SIGNATURE_LINK_START.len_utf8();
        let Some(relative_separator) = source[key_start..].find(SIGNATURE_LINK_SEPARATOR) else {
            output.push(SIGNATURE_LINK_START);
            cursor = key_start;
            continue;
        };
        let separator = key_start + relative_separator;
        let label_start = separator + SIGNATURE_LINK_SEPARATOR.len_utf8();
        let Some(relative_end) = source[label_start..].find(SIGNATURE_LINK_END) else {
            output.push(SIGNATURE_LINK_START);
            cursor = key_start;
            continue;
        };
        let end = label_start + relative_end;
        let key = &source[key_start..separator];
        let label = &source[label_start..end];
        let target = api
            .signature_targets
            .get(key)
            .and_then(|target| resolve_signature_link(target, route_by_id, workspace_routes));
        if let Some(target) = target.as_ref() {
            output.push(SIGNATURE_LINK_START);
            output.push_str(target.marker);
            output.push(':');
            output.push_str(target.href);
            output.push(SIGNATURE_LINK_SEPARATOR);
        }
        output.push_str(label);
        if target.is_some() {
            output.push(SIGNATURE_LINK_END);
        }
        cursor = end + SIGNATURE_LINK_END.len_utf8();
    }
    output.push_str(&source[cursor..]);
    let mut rewritten = String::with_capacity(frontmatter.len() + output.len());
    rewritten.push_str(frontmatter);
    rewritten.push_str(&output);
    *markdown = rewritten;
}

struct ResolvedSignatureLink<'a> {
    marker: &'static str,
    href: &'a str,
}

fn resolve_signature_link<'a>(
    target: &'a SignatureLinkTarget,
    route_by_id: &'a ItemRoutes,
    workspace_routes: &'a WorkspaceRoutes,
) -> Option<ResolvedSignatureLink<'a>> {
    if let Some(route) = target.local_id.as_ref().and_then(|id| route_by_id.get(id)) {
        return Some(ResolvedSignatureLink {
            marker: "L",
            href: route,
        });
    }

    if let Some(route) = target
        .crate_name
        .as_ref()
        .zip(target.rust_path.as_ref())
        .and_then(|(crate_name, path)| workspace_routes.get(crate_name, path))
    {
        return Some(ResolvedSignatureLink {
            marker: "L",
            href: route,
        });
    }

    target
        .external_url
        .as_deref()
        .map(|href| ResolvedSignatureLink { marker: "E", href })
}

fn split_frontmatter(markdown: &str) -> (&str, &str) {
    let Some(after_opening) = markdown.strip_prefix("---\n") else {
        return ("", markdown);
    };
    let Some(end) = after_opening.find("\n---\n") else {
        return ("", markdown);
    };
    let body_start = 4 + end + "\n---\n".len();
    markdown.split_at(body_start)
}

pub(super) fn rewrite_doc_link_groups(
    markdown: &str,
    groups: &[DocLinkGroup],
    route_by_id: &ItemRoutes,
    workspace_routes: &WorkspaceRoutes,
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
        rewrite_link_tokens(&mut rewritten, &group.links, route_by_id, workspace_routes);
        output.push_str(&rewritten);
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    output
}

fn rewrite_link_tokens(
    output: &mut String,
    links: &[(String, DocLinkTarget)],
    route_by_id: &ItemRoutes,
    workspace_routes: &WorkspaceRoutes,
) {
    for (label, target) in links {
        let Some(resolved) = resolve_doc_link(target, route_by_id, workspace_routes) else {
            continue;
        };

        *output = rewrite_named_reference_link_destinations(
            output,
            label,
            resolved.href,
            resolved.external_kind,
        );
        *output =
            rewrite_named_link_destinations(output, label, resolved.href, resolved.external_kind);

        let labels = if label.starts_with('`') && label.ends_with('`') {
            vec![(format!("[{label}]"), true)]
        } else {
            vec![
                (format!("[{label}]"), false),
                (format!("[`{label}`]"), true),
            ]
        };
        for (token, code_label) in labels {
            let replacement = if let Some(kind) = resolved.external_kind {
                let display_label = if code_label {
                    format!("<code>{}</code>", html_escape_text(label.trim_matches('`')))
                } else {
                    html_escape_text(label)
                };
                external_api_link(resolved.href, kind, &display_label)
            } else {
                format!("{token}({})", resolved.href)
            };
            *output = replace_shortcut_link(output, &token, &replacement);
        }
    }
}

struct ResolvedDocLink<'a> {
    href: &'a str,
    external_kind: Option<&'a str>,
}

fn resolve_doc_link<'a>(
    target: &'a DocLinkTarget,
    route_by_id: &'a ItemRoutes,
    workspace_routes: &'a WorkspaceRoutes,
) -> Option<ResolvedDocLink<'a>> {
    match target {
        DocLinkTarget::Item(target_id) => route_by_id.get(target_id).map(|route| ResolvedDocLink {
            href: route,
            external_kind: None,
        }),
        DocLinkTarget::Url {
            href,
            kind,
            crate_name,
            rust_path,
        } => {
            if let Some(route) = workspace_routes.get(crate_name, rust_path) {
                Some(ResolvedDocLink {
                    href: route,
                    external_kind: None,
                })
            } else {
                Some(ResolvedDocLink {
                    href,
                    external_kind: Some(kind),
                })
            }
        }
    }
}

fn rewrite_named_reference_link_destinations(
    markdown: &str,
    name: &str,
    href: &str,
    external_kind: Option<&str>,
) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut in_fenced_code = false;
    let mut reference_labels = Vec::new();
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
        if leading_space > 3 {
            output.push_str(line);
            continue;
        }
        let definition = &line_without_ending[leading_space..];
        if !definition.starts_with('[') {
            output.push_str(line);
            continue;
        }
        let Some(label_end) = definition.find("]:") else {
            output.push_str(line);
            continue;
        };
        let destination_start = label_end + 2;
        let destination_with_space = &definition[destination_start..];
        let whitespace = destination_with_space.len() - destination_with_space.trim_start().len();
        let destination = &destination_with_space[whitespace..];
        let Some(target) = markdown_link_destination(destination) else {
            output.push_str(line);
            continue;
        };
        if target.href != name {
            output.push_str(line);
            continue;
        }

        reference_labels.push(definition[1..label_end].to_owned());
        let target_start = leading_space + destination_start + whitespace + target.start;
        let target_end = leading_space + destination_start + whitespace + target.end;
        output.push_str(&line_without_ending[..target_start]);
        output.push_str(href);
        output.push_str(&line_without_ending[target_end..]);
        if line.ends_with('\n') {
            output.push('\n');
        }
    }

    for label in reference_labels {
        let token = format!("[{label}]");
        let replacement = if let Some(kind) = external_kind {
            let display_label = if label.starts_with('`') && label.ends_with('`') {
                format!("<code>{}</code>", html_escape_text(label.trim_matches('`')))
            } else {
                html_escape_text(&label)
            };
            external_api_link(href, kind, &display_label)
        } else {
            format!("{token}({href})")
        };
        output = replace_shortcut_link(&output, &token, &replacement);
    }

    output
}

fn rewrite_named_link_destinations(
    markdown: &str,
    name: &str,
    href: &str,
    external_kind: Option<&str>,
) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut cursor = 0;
    while let Some(relative_end) = markdown[cursor..].find("](") {
        let label_end = cursor + relative_end;
        let Some(relative_start) = markdown[..label_end].rfind('[') else {
            output.push_str(&markdown[cursor..]);
            return output;
        };
        let label_start = relative_start + 1;
        let open_paren = label_end + 1;
        let Some(link_end) = markdown_link_end(markdown, open_paren) else {
            output.push_str(&markdown[cursor..]);
            return output;
        };
        let destination = markdown_link_destination(&markdown[open_paren + 1..link_end]);
        if markdown[..relative_start].ends_with('!')
            || inside_api_signature(markdown, relative_start)
            || destination.is_none_or(|destination| destination.href != name)
        {
            let next_cursor = label_end + 2;
            output.push_str(&markdown[cursor..next_cursor]);
            cursor = next_cursor;
            continue;
        }

        output.push_str(&markdown[cursor..relative_start]);
        if let Some(kind) = external_kind {
            let link_label = &markdown[label_start..label_end];
            let display_label = if link_label.starts_with('`') && link_label.ends_with('`') {
                format!(
                    "<code>{}</code>",
                    html_escape_text(link_label.trim_matches('`'))
                )
            } else {
                html_escape_text(link_label)
            };
            output.push_str(&external_api_link(href, kind, &display_label));
        } else {
            let destination = destination.expect("matched destination must be present");
            let destination_start = open_paren + 1 + destination.start;
            let destination_end = open_paren + 1 + destination.end;
            output.push_str(&markdown[relative_start..destination_start]);
            output.push_str(href);
            output.push_str(&markdown[destination_end..=link_end]);
        }
        cursor = link_end + 1;
    }
    output.push_str(&markdown[cursor..]);
    output
}

fn external_api_link(href: &str, kind: &str, label: &str) -> String {
    let href = html_escape_attribute(href);
    let kind = html_escape_attribute(kind);
    format!(
        "<a className=\"api-reference-link\" href=\"{href}\" data-kind=\"{kind}\" target=\"_blank\" rel=\"noopener noreferrer\"><ApiItemMark compact kind=\"{kind}\" />{label}</a>"
    )
}

#[derive(Clone, Copy)]
struct LinkDestination<'a> {
    start: usize,
    end: usize,
    href: &'a str,
}

fn markdown_link_destination(destination: &str) -> Option<LinkDestination<'_>> {
    let leading_whitespace = destination.len() - destination.trim_start().len();
    let destination = destination.trim_start();
    if let Some(destination) = destination.strip_prefix('<') {
        let (href, _) = destination.split_once('>')?;
        let start = leading_whitespace;
        return Some(LinkDestination {
            start,
            end: start + href.len() + 2,
            href,
        });
    }
    let href = destination.split_whitespace().next()?;
    let start = leading_whitespace;
    Some(LinkDestination {
        start,
        end: start + href.len(),
        href,
    })
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

pub(super) fn mdx_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '{' => escaped.push_str("&#123;"),
            '}' => escaped.push_str("&#125;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn html_escape_attribute(value: &str) -> String {
    html_escape_text(value).replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_complete_frontmatter_only() {
        let markdown = "---\ntitle: Demo\n---\n\nBody";
        let (frontmatter, body) = split_frontmatter(markdown);

        assert_eq!(frontmatter, "---\ntitle: Demo\n---\n");
        assert_eq!(body, "\nBody");
        assert_eq!(
            split_frontmatter("---\nmissing end"),
            ("", "---\nmissing end")
        );
    }

    #[test]
    fn parses_destinations_with_titles_and_angle_brackets() {
        assert_eq!(
            markdown_link_destination(" https://example.com \"Title\"")
                .map(|destination| destination.href),
            Some("https://example.com")
        );
        assert_eq!(
            markdown_link_destination("<https://example.com/a b> 'Title'")
                .map(|destination| destination.href),
            Some("https://example.com/a b")
        );
    }

    #[test]
    fn finds_balanced_link_destinations() {
        let markdown = "[label](https://example.com/a_(b)) suffix";
        let open = markdown.find('(').unwrap();

        assert_eq!(markdown_link_end(markdown, open), markdown.rfind(')'));
    }

    #[test]
    fn leaves_reference_links_inside_code_fences_unchanged() {
        let markdown = "```md\n[label]: [site](https://example.com)\n```\n\n[label]: [site](https://example.com)\n";
        let normalized = normalize_reference_link_destinations(markdown);

        assert!(normalized.starts_with("```md\n[label]: [site](https://example.com)"));
        assert!(normalized.ends_with("[label]: <https://example.com>\n"));
    }

    #[test]
    fn escapes_mdx_and_html_text() {
        assert_eq!(mdx_text("A <B> {C}"), "A &lt;B&gt; &#123;C&#125;");
        assert_eq!(html_escape_attribute("a&\"<b>"), "a&amp;&quot;&lt;b&gt;");
    }
}
