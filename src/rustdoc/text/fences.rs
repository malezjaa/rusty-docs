pub(crate) fn normalize_code_fences(markdown: &str) -> String {
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

        let normalized = if info == "none" {
            Some(("text", String::new()))
        } else {
            normalize_doctest_info(info).map(|info| ("rust", info))
        };

        if let Some((language, info)) = normalized {
            output.push_str(fence.prefix);
            output.push_str(language);
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

pub(super) struct Fence<'a> {
    prefix: &'a str,
    pub(super) marker: char,
    pub(super) length: usize,
}

pub(super) fn opening_fence(line: &str) -> Option<(Fence<'_>, &str)> {
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

pub(super) fn closes_fence(line: &str, marker: char, minimum_length: usize) -> bool {
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
    let title = crate::formatting::json_string(&label);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_rust_doctest_modifiers() {
        assert_eq!(
            normalize_code_fences("```rust,no_run\nfn main() {}\n```\n"),
            "```rust title=\"Compiles without running\"\nfn main() {}\n```\n"
        );
    }

    #[test]
    fn preserves_non_rust_fences() {
        let markdown = "```typescript\nconst value = 1;\n```\n";
        assert_eq!(normalize_code_fences(markdown), markdown);
    }
}
