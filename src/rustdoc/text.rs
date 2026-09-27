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
    let mut line_index = 0;

    while let Some(line) = lines.get(line_index).copied() {
        let without_newline = line.strip_suffix('\n').unwrap_or(line);
        let (content, line_ending) = without_newline
            .strip_suffix('\r')
            .map_or((without_newline, ""), |content| (content, "\r"));

        if let Some((marker, length)) = open_fence {
            if closes_fence(content, marker, length) {
                open_fence = None;
            }
            output.push_str(line);
            line_index += 1;
            continue;
        }

        if let Some((fence, _)) = opening_fence(content) {
            open_fence = Some((fence.marker, fence.length));
            output.push_str(line);
            line_index += 1;
            continue;
        }

        let indent = content.bytes().take_while(|byte| *byte == b' ').count();
        let setext_level = lines.get(line_index + 1).and_then(|next_line| {
            let next = next_line.strip_suffix('\n').unwrap_or(next_line);
            let next = next.strip_suffix('\r').unwrap_or(next);
            let next_indent = next.bytes().take_while(|byte| *byte == b' ').count();
            let underline = next.get(next_indent..)?.trim();
            let title = content.get(indent..)?.trim();
            if indent > 3
                || next_indent > 3
                || title.is_empty()
                || title.starts_with('#')
                || title
                    .chars()
                    .all(|character| matches!(character, '-' | '='))
                || underline.is_empty()
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
        });

        if let Some(level) = setext_level {
            output.push_str(&content[..indent]);
            output.push_str(&"#".repeat((level + parent_heading_level).min(6)));
            output.push(' ');
            output.push_str(content[indent..].trim_end());
            output.push_str(line_ending);
            if line.ends_with('\n') {
                output.push('\n');
            }
            line_index += 2;
            continue;
        }

        let heading = content.get(indent..).and_then(|value| {
            let level = value.bytes().take_while(|byte| *byte == b'#').count();
            let boundary = value.as_bytes().get(level);
            (indent <= 3
                && (1..=6).contains(&level)
                && boundary.is_none_or(u8::is_ascii_whitespace))
            .then_some(level)
        });

        if let Some(level) = heading {
            let target_level = (level + parent_heading_level).min(6);
            output.push_str(&content[..indent]);
            output.push_str(&"#".repeat(target_level));
            output.push_str(&content[indent + level..]);
            output.push_str(line_ending);
            if line.ends_with('\n') {
                output.push('\n');
            }
        } else {
            output.push_str(line);
        }
        line_index += 1;
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
    let mut line_index = 0;

    while let Some(line) = lines.get(line_index).copied() {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let content = content.strip_suffix('\r').unwrap_or(content);

        if let Some((marker, length)) = open_fence {
            if closes_fence(content, marker, length) {
                open_fence = None;
            }
            line_index += 1;
            continue;
        }

        if let Some((fence, _)) = opening_fence(content) {
            open_fence = Some((fence.marker, fence.length));
            line_index += 1;
            continue;
        }

        let indent = content.bytes().take_while(|byte| *byte == b' ').count();
        let setext_level = lines.get(line_index + 1).and_then(|next_line| {
            let next = next_line.strip_suffix('\n').unwrap_or(next_line);
            let next = next.strip_suffix('\r').unwrap_or(next);
            let next_indent = next.bytes().take_while(|byte| *byte == b' ').count();
            let underline = next.get(next_indent..)?.trim();
            let title = content.get(indent..)?.trim();
            if indent > 3
                || next_indent > 3
                || title.is_empty()
                || title.starts_with('#')
                || title
                    .chars()
                    .all(|character| matches!(character, '-' | '='))
                || underline.is_empty()
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
        });

        if setext_level.is_some() {
            count += 1;
            line_index += 2;
            continue;
        }

        let heading = content.get(indent..).and_then(|value| {
            let level = value.bytes().take_while(|byte| *byte == b'#').count();
            let boundary = value.as_bytes().get(level);
            (indent <= 3
                && (1..=6).contains(&level)
                && boundary.is_none_or(u8::is_ascii_whitespace))
            .then_some(level)
        });

        if heading.is_some() {
            count += 1;
        }
        line_index += 1;
    }

    count
}

pub(super) fn normalize_doctest_fences(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut open_fence = None;

    for line in markdown.split_inclusive('\n') {
        let without_newline = line.strip_suffix('\n').unwrap_or(line);
        let (content, line_ending) = without_newline
            .strip_suffix('\r')
            .map_or((without_newline, ""), |content| (content, "\r"));

        if let Some((marker, length)) = open_fence {
            if closes_fence(content, marker, length) {
                open_fence = None;
            }
            output.push_str(line);
            continue;
        }

        let Some((fence, info)) = opening_fence(content) else {
            output.push_str(line);
            continue;
        };

        if let Some(info) = normalize_doctest_info(info) {
            output.push_str(fence.prefix);
            output.push_str("rust");
            output.push_str(&info);
            output.push_str(line_ending);
            if line.ends_with('\n') {
                output.push('\n');
            }
        } else {
            output.push_str(line);
        }
        open_fence = Some((fence.marker, fence.length));
    }

    output
}

struct Fence<'a> {
    prefix: &'a str,
    marker: char,
    length: usize,
}

fn opening_fence(line: &str) -> Option<(Fence<'_>, &str)> {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    if indent > 3 {
        return None;
    }

    let marker = line[indent..].chars().next()?;
    if !matches!(marker, '`' | '~') {
        return None;
    }

    let length = line[indent..]
        .chars()
        .take_while(|character| *character == marker)
        .count();
    if length < 3 {
        return None;
    }

    let info_start = indent + length;
    let info = line[info_start..].trim();
    if marker == '`' && info.contains('`') {
        return None;
    }

    Some((
        Fence {
            prefix: &line[..info_start],
            marker,
            length,
        },
        info,
    ))
}

fn closes_fence(line: &str, marker: char, minimum_length: usize) -> bool {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    if indent > 3 {
        return false;
    }

    let fence = &line[indent..];
    let length = fence
        .chars()
        .take_while(|character| *character == marker)
        .count();
    length >= minimum_length && fence[length..].trim().is_empty()
}

fn normalize_doctest_info(info: &str) -> Option<String> {
    if info.is_empty() {
        return Some(String::new());
    }
    let (info, comment) = strip_rustdoc_comment(info);
    let tokens = split_fence_info(&info);
    let first = *tokens.first()?;
    if first != "rust" && !is_doctest_modifier(first) {
        return None;
    }

    let mut modifiers = Vec::new();
    let mut other_tokens = Vec::new();
    for token in tokens {
        if token == "rust" {
            continue;
        }
        if is_doctest_modifier(token) {
            modifiers.push(token);
        } else {
            other_tokens.push(token);
        }
    }
    if modifiers.is_empty() {
        return None;
    }

    let mut labels = modifiers
        .iter()
        .map(|modifier| doctest_modifier_label(modifier))
        .collect::<Vec<_>>();
    if let Some((title_index, title)) = other_tokens
        .iter()
        .enumerate()
        .find_map(|(index, token)| code_title(token).map(|title| (index, title)))
    {
        other_tokens.remove(title_index);
        labels.push(title);
    }

    let mut label = labels.join(", ");
    if let Some(comment) = comment {
        label.push_str(": ");
        label.push_str(&comment);
    }
    let title = serde_json::to_string(&label).unwrap_or_else(|_| "\"Rust doctest\"".into());
    let mut output = format!(" title={title}");
    if !other_tokens.is_empty() {
        output.push(' ');
        output.push_str(&other_tokens.join(" "));
    }
    Some(output)
}

fn strip_rustdoc_comment(info: &str) -> (String, Option<String>) {
    let mut quote = None;
    let mut escaped = false;
    let mut start = None;

    for (index, character) in info.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
        } else if character == '(' {
            start = Some(index);
            break;
        }
    }

    let Some(start) = start else {
        return (info.to_owned(), None);
    };

    let mut depth = 0;
    let mut end = None;
    for (offset, character) in info[start..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + offset);
                    break;
                }
            }
            _ => {}
        }
    }

    let Some(end) = end else {
        return (info.to_owned(), None);
    };

    let comment = info[start + 1..end].trim();
    let mut cleaned = String::with_capacity(info.len());
    cleaned.push_str(info[..start].trim_end());
    cleaned.push(' ');
    cleaned.push_str(info[end + 1..].trim_start());
    (cleaned, (!comment.is_empty()).then(|| comment.to_owned()))
}

fn split_fence_info(info: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = None;
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in info.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
        } else if character == ',' || character.is_whitespace() {
            if let Some(token_start) = start.take() {
                tokens.push(&info[token_start..index]);
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(token_start) = start {
        tokens.push(&info[token_start..]);
    }
    tokens
}

fn is_doctest_modifier(value: &str) -> bool {
    matches!(
        value,
        "ignore"
            | "no_run"
            | "should_panic"
            | "compile_fail"
            | "edition2015"
            | "edition2018"
            | "edition2021"
            | "edition2024"
            | "test_harness"
            | "standalone_crate"
    ) || value.starts_with("ignore-")
}

fn doctest_modifier_label(value: &str) -> String {
    match value {
        "ignore" => "Ignored test".to_owned(),
        "no_run" => "Compiles without running".to_owned(),
        "should_panic" => "Should panic".to_owned(),
        "compile_fail" => "Must fail to compile".to_owned(),
        "edition2015" => "Rust 2015 edition".to_owned(),
        "edition2018" => "Rust 2018 edition".to_owned(),
        "edition2021" => "Rust 2021 edition".to_owned(),
        "edition2024" => "Rust 2024 edition".to_owned(),
        "test_harness" => "Uses the test harness".to_owned(),
        "standalone_crate" => "Standalone crate".to_owned(),
        value if value.starts_with("ignore-") => format!("Ignored for {}", &value[7..]),
        _ => "Rust doctest".to_owned(),
    }
}

fn code_title(token: &str) -> Option<String> {
    let title = token.strip_prefix("title=")?;
    let quoted_title = title
        .strip_prefix('"')
        .and_then(|title| title.strip_suffix('"'))
        .or_else(|| {
            title
                .strip_prefix('\'')
                .and_then(|title| title.strip_suffix('\''))
        });
    Some(quoted_title.unwrap_or(title).to_owned())
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
            let is_html_tag = matches!(
                name.as_str(),
                "a" | "b"
                    | "blockquote"
                    | "br"
                    | "code"
                    | "dd"
                    | "del"
                    | "div"
                    | "dl"
                    | "dt"
                    | "em"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "hr"
                    | "i"
                    | "img"
                    | "li"
                    | "ol"
                    | "p"
                    | "pre"
                    | "s"
                    | "span"
                    | "strong"
                    | "sub"
                    | "sup"
                    | "table"
                    | "tbody"
                    | "td"
                    | "th"
                    | "thead"
                    | "tr"
                    | "u"
                    | "ul"
            );
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

pub(super) fn yaml_quote(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
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
        "item".to_owned()
    } else {
        slug
    }
}
