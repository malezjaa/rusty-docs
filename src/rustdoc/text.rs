mod fences;

use fences::{closes_fence, opening_fence};

pub(super) use fences::normalize_code_fences;

pub(super) fn first_sentence(docs: &str) -> Option<String> {
    let line = docs
        .lines()
        .find(|line| !line.trim().is_empty())?
        .trim()
        .trim_start_matches('#')
        .trim();
    if line.is_empty() {
        return None;
    }
    let plain = markdown_plain_text(line);
    let first = plain.split(". ").next().unwrap_or(&plain).trim();
    if first.is_empty() {
        return None;
    }
    let short = first.chars().take(180).collect::<String>();
    Some(short)
}

pub(super) fn shift_doc_headings(markdown: &str, parent_heading_level: usize) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut open_fence = None;
    let lines = markdown.split_inclusive('\n').collect::<Vec<_>>();
    let mut index = 0;

    while let Some(raw_line) = lines.get(index).copied() {
        let line = MarkdownLine::new(raw_line);

        if let Some((marker, length)) = open_fence {
            if closes_fence(line.content, marker, length) {
                open_fence = None;
            }
            output.push_str(raw_line);
            index += 1;
            continue;
        }

        if let Some((fence, _)) = opening_fence(line.content) {
            open_fence = Some((fence.marker, fence.length));
            output.push_str(raw_line);
            index += 1;
            continue;
        }

        match heading_at(&lines, index) {
            Some(Heading::Setext { level }) => {
                write_heading_prefix(&mut output, &line, level, parent_heading_level);
                output.push(' ');
                output.push_str(line.text().trim_end());
                output.push_str(line.ending);
                if line.has_newline {
                    output.push('\n');
                }
                index += 2;
            }
            Some(Heading::Atx { level }) => {
                write_heading_prefix(&mut output, &line, level, parent_heading_level);
                output.push_str(&line.text()[level..]);
                output.push_str(line.ending);
                if line.has_newline {
                    output.push('\n');
                }
                index += 1;
            }
            None => {
                output.push_str(raw_line);
                index += 1;
            }
        }
    }

    output
}

pub(super) fn heading_count(markdown: &str) -> usize {
    let markdown = markdown
        .strip_prefix("---\n")
        .and_then(|body| body.split_once("\n---\n").map(|(_, body)| body))
        .unwrap_or(markdown);
    let lines = markdown.split_inclusive('\n').collect::<Vec<_>>();
    let mut count = 0;
    let mut open_fence = None;
    let mut index = 0;

    while let Some(raw_line) = lines.get(index).copied() {
        let line = MarkdownLine::new(raw_line);

        if let Some((marker, length)) = open_fence {
            if closes_fence(line.content, marker, length) {
                open_fence = None;
            }
            index += 1;
            continue;
        }

        if let Some((fence, _)) = opening_fence(line.content) {
            open_fence = Some((fence.marker, fence.length));
            index += 1;
            continue;
        }

        if let Some(heading) = heading_at(&lines, index) {
            count += 1;
            index += heading.line_count();
        } else {
            index += 1;
        }
    }

    count
}

#[derive(Clone, Copy)]
enum Heading {
    Atx { level: usize },
    Setext { level: usize },
}

impl Heading {
    fn line_count(self) -> usize {
        match self {
            Self::Atx { .. } => 1,
            Self::Setext { .. } => 2,
        }
    }
}

struct MarkdownLine<'a> {
    content: &'a str,
    ending: &'a str,
    indent: usize,
    has_newline: bool,
}

impl<'a> MarkdownLine<'a> {
    fn new(raw: &'a str) -> Self {
        let has_newline = raw.ends_with('\n');
        let without_newline = raw.strip_suffix('\n').unwrap_or(raw);
        let (content, ending) = without_newline
            .strip_suffix('\r')
            .map_or((without_newline, ""), |content| (content, "\r"));
        let indent = content.bytes().take_while(|byte| *byte == b' ').count();

        Self {
            content,
            ending,
            indent,
            has_newline,
        }
    }

    fn text(&self) -> &'a str {
        &self.content[self.indent..]
    }
}

fn heading_at(lines: &[&str], index: usize) -> Option<Heading> {
    let line = MarkdownLine::new(lines.get(index)?);
    if let Some(next) = lines.get(index + 1) {
        let next = MarkdownLine::new(next);
        if let Some(level) = setext_level(&line, &next) {
            return Some(Heading::Setext { level });
        }
    }

    let level = line.text().bytes().take_while(|byte| *byte == b'#').count();
    let boundary = line.text().as_bytes().get(level);
    (line.indent <= 3 && (1..=6).contains(&level) && boundary.is_none_or(u8::is_ascii_whitespace))
        .then_some(Heading::Atx { level })
}

fn setext_level(line: &MarkdownLine<'_>, underline_line: &MarkdownLine<'_>) -> Option<usize> {
    let title = line.text().trim();
    let underline = underline_line.text().trim();
    if line.indent > 3
        || underline_line.indent > 3
        || underline.is_empty()
        || title.is_empty()
        || title.starts_with('#')
        || title
            .chars()
            .all(|character| matches!(character, '-' | '='))
    {
        return None;
    }

    if underline.chars().all(|character| character == '=') {
        Some(1)
    } else if underline.chars().all(|character| character == '-') {
        Some(2)
    } else {
        None
    }
}

fn write_heading_prefix(
    output: &mut String,
    line: &MarkdownLine<'_>,
    level: usize,
    parent_level: usize,
) {
    output.push_str(&line.content[..line.indent]);
    output.push_str(&"#".repeat((level + parent_level).min(6)));
}

fn markdown_plain_text(markdown: &str) -> String {
    let chars = markdown.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '['
            && let Some(label_end) = chars[index + 1..].iter().position(|ch| *ch == ']')
        {
            let label_start = index + 1;
            let label_end = label_start + label_end;
            let label = chars[label_start..label_end]
                .iter()
                .copied()
                .collect::<String>();
            output.push_str(&markdown_plain_text(&label));
            index = label_end + 1;
            if chars.get(index) == Some(&'(') {
                let mut depth = 1;
                index += 1;
                while index < chars.len() && depth > 0 {
                    match chars[index] {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                    index += 1;
                }
            }
            continue;
        }

        if chars[index] == '!' && chars.get(index + 1) == Some(&'[') {
            index += 1;
            continue;
        }

        if chars[index] == '`' || chars[index] == '*' || chars[index] == '_' || chars[index] == '~'
        {
            index += 1;
            continue;
        }

        if chars[index] == '<' {
            let name_start = if chars.get(index + 1) == Some(&'/') {
                index + 2
            } else {
                index + 1
            };
            let name = chars[name_start..]
                .iter()
                .take_while(|ch| ch.is_ascii_alphanumeric())
                .copied()
                .collect::<String>()
                .to_ascii_lowercase();
            let is_html_tag = HTML_TAGS.contains(&name.as_str());
            let tag_boundary = chars
                .get(name_start + name.chars().count())
                .is_some_and(|ch| ch.is_whitespace() || *ch == '>' || *ch == '/');
            if is_html_tag
                && tag_boundary
                && let Some(tag_end) = chars[index..].iter().position(|ch| *ch == '>')
            {
                index += tag_end + 1;
                continue;
            }
        }

        if chars[index] == '\\' && index + 1 < chars.len() {
            index += 1;
        }

        output.push(chars[index]);
        index += 1;
    }

    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

const HTML_TAGS: &[&str] = &[
    "a",
    "b",
    "blockquote",
    "br",
    "code",
    "dd",
    "del",
    "div",
    "dl",
    "dt",
    "em",
    "h1",
    "h2",
    "h3",
    "h4",
    "hr",
    "i",
    "img",
    "li",
    "ol",
    "p",
    "pre",
    "s",
    "span",
    "strong",
    "sub",
    "sup",
    "table",
    "tbody",
    "td",
    "th",
    "thead",
    "tr",
    "u",
    "ul",
];

pub(super) fn yaml_quote(value: &str) -> String {
    crate::formatting::json_string(value)
}

pub(super) fn slug(value: &str) -> String {
    crate::formatting::slug(value, "item")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifts_atx_and_setext_headings() {
        let markdown = "# First\n\nSecond\n------\n";

        assert_eq!(
            shift_doc_headings(markdown, 2),
            "### First\n\n#### Second\n"
        );
    }

    #[test]
    fn ignores_headings_inside_code_fences() {
        let markdown = "## Visible\n\n```md\n# Code\nHidden\n------\n```\n";

        assert_eq!(heading_count(markdown), 1);
        assert!(shift_doc_headings(markdown, 1).contains("```md\n# Code\nHidden\n------\n```"));
    }

    #[test]
    fn excludes_frontmatter_from_heading_counts() {
        let markdown = "---\ntitle: Example\n---\n\n# Page\n\nSection\n=======\n";

        assert_eq!(heading_count(markdown), 2);
    }

    #[test]
    fn extracts_plain_first_sentence() {
        assert_eq!(
            first_sentence("**Fast** [`Widget`](https://example.com). More details."),
            Some("Fast Widget".to_owned())
        );
    }
}
