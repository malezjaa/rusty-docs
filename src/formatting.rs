pub(crate) fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string as JSON cannot fail")
}

pub(crate) fn slug(value: &str, fallback: &str) -> String {
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
        fallback.to_owned()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_normalizes_separators_and_case() {
        assert_eq!(slug("Hello, Rust_world!", "item"), "hello-rust-world");
    }

    #[test]
    fn slug_uses_fallback_for_empty_values() {
        assert_eq!(slug("---", "package"), "package");
    }

    #[test]
    fn json_string_escapes_content() {
        assert_eq!(json_string("line\n\"quoted\""), "\"line\\n\\\"quoted\\\"\"");
    }
}
