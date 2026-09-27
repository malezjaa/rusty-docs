use serde_json::Value;

use super::{ApiTocItem, DocLinkGroup, DocLinkTarget, member_anchor, signature, text};

pub(super) fn doc_link_group(markdown: &str, item: &Value) -> DocLinkGroup {
    let links = item
        .get("links")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(text, target)| {
            Some((
                text.clone(),
                DocLinkTarget::Item(target.as_u64()?.to_string()),
            ))
        })
        .collect();
    DocLinkGroup {
        markdown: markdown.to_owned(),
        links,
    }
}

pub(super) fn append_members(
    markdown: &mut String,
    kind: &str,
    item: &Value,
    index: &serde_json::Map<String, Value>,
    doc_links: &mut Vec<DocLinkGroup>,
) -> Vec<ApiTocItem> {
    let mut api_toc = Vec::new();
    let mut heading_count = text::heading_count(markdown);
    match kind {
        "struct" | "union" => append_fields(
            markdown,
            item,
            index,
            kind,
            &mut heading_count,
            &mut api_toc,
            doc_links,
        ),
        "enum" => append_variants(
            markdown,
            item,
            index,
            &mut heading_count,
            &mut api_toc,
            doc_links,
        ),
        "trait" => append_trait_members(
            markdown,
            item,
            index,
            &mut heading_count,
            &mut api_toc,
            doc_links,
        ),
        _ => {}
    }
    if matches!(kind, "struct" | "enum" | "union") {
        append_implementations(
            markdown,
            item,
            index,
            &mut heading_count,
            &mut api_toc,
            doc_links,
        );
    }
    api_toc
}

fn append_fields(
    markdown: &mut String,
    item: &Value,
    index: &serde_json::Map<String, Value>,
    kind: &str,
    heading_count: &mut usize,
    api_toc: &mut Vec<ApiTocItem>,
    doc_links: &mut Vec<DocLinkGroup>,
) {
    let data = item.get("inner").and_then(|inner| inner.get(kind));
    let shape = data.and_then(|data| data.get("kind"));
    let plain_fields = shape
        .and_then(|shape| shape.get("plain"))
        .and_then(|plain| plain.get("fields"))
        .and_then(Value::as_array);
    let tuple_fields = shape
        .and_then(|shape| shape.get("tuple"))
        .and_then(Value::as_array);
    let fields = plain_fields.or(tuple_fields);
    let Some(fields) = fields else { return };
    let mut rendered = Vec::new();
    for (field_index, id) in fields.iter().enumerate() {
        let Some(field) = signature::get_item(index, id) else {
            continue;
        };
        if field.get("visibility").and_then(Value::as_str) != Some("public") {
            continue;
        }
        let signature = if let Some(name) = field.get("name").and_then(Value::as_str) {
            format!(
                "{}{name}: {}",
                signature::visibility(field),
                signature::field_type(field)
            )
        } else {
            format!(
                "{}{}",
                signature::visibility(field),
                signature::field_type(field)
            )
        };
        rendered.push((
            field_index,
            signature,
            field.get("docs").and_then(Value::as_str),
            field,
        ));
    }
    if rendered.is_empty() {
        return;
    }
    markdown.push_str("## Fields\n\n");
    *heading_count += 1;
    for (field_index, signature, docs, field) in rendered {
        let name = field
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| field_index.to_string());
        append_api_toc_member(
            markdown,
            api_toc,
            heading_count,
            doc_links,
            "field",
            &name,
            &signature,
            docs,
            2,
            3,
            Some(field),
            true,
            1,
        );
    }
}

fn append_variants(
    markdown: &mut String,
    item: &Value,
    index: &serde_json::Map<String, Value>,
    heading_count: &mut usize,
    api_toc: &mut Vec<ApiTocItem>,
    doc_links: &mut Vec<DocLinkGroup>,
) {
    let Some(variants) = item
        .get("inner")
        .and_then(|inner| inner.get("enum"))
        .and_then(|data| data.get("variants"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let variants = variants
        .iter()
        .filter_map(|id| signature::get_item(index, id))
        .collect::<Vec<_>>();
    if variants.is_empty() {
        return;
    }
    markdown.push_str("## Variants\n\n");
    *heading_count += 1;
    for variant in variants {
        let name = variant
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("variant");
        let variant_signature = signature::variant_signature(name, variant, index);
        append_api_toc_member(
            markdown,
            api_toc,
            heading_count,
            doc_links,
            "variant",
            name,
            &variant_signature,
            variant.get("docs").and_then(Value::as_str),
            2,
            3,
            Some(variant),
            true,
            1,
        );
        append_documented_variant_fields(
            markdown,
            variant,
            index,
            heading_count,
            api_toc,
            doc_links,
        );
    }
}

fn append_documented_variant_fields(
    markdown: &mut String,
    variant: &Value,
    index: &serde_json::Map<String, Value>,
    heading_count: &mut usize,
    api_toc: &mut Vec<ApiTocItem>,
    doc_links: &mut Vec<DocLinkGroup>,
) {
    let shape = variant
        .get("inner")
        .and_then(|inner| inner.get("variant"))
        .and_then(|data| data.get("kind"));
    let fields = shape
        .and_then(|shape| shape.get("struct"))
        .and_then(|value| value.get("fields"))
        .or_else(|| shape.and_then(|shape| shape.get("tuple")));
    let Some(fields) = fields.and_then(Value::as_array) else {
        return;
    };
    for (field_index, id) in fields.iter().enumerate() {
        let Some(field) = signature::get_item(index, id) else {
            continue;
        };
        let Some(docs) = field
            .get("docs")
            .and_then(Value::as_str)
            .filter(|docs| !docs.trim().is_empty())
        else {
            continue;
        };
        let name = field
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| field_index.to_string());
        append_api_toc_member(
            markdown,
            api_toc,
            heading_count,
            doc_links,
            "field",
            &name,
            &name,
            Some(docs),
            3,
            4,
            Some(field),
            true,
            1,
        );
    }
}

fn append_trait_members(
    markdown: &mut String,
    item: &Value,
    index: &serde_json::Map<String, Value>,
    heading_count: &mut usize,
    api_toc: &mut Vec<ApiTocItem>,
    doc_links: &mut Vec<DocLinkGroup>,
) {
    let Some(items) = item
        .get("inner")
        .and_then(|inner| inner.get("trait"))
        .and_then(|data| data.get("items"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let members = items
        .iter()
        .filter_map(|id| signature::get_item(index, id))
        .collect::<Vec<_>>();
    if members.is_empty() {
        return;
    }
    markdown.push_str("## Required and provided items\n\n");
    *heading_count += 1;
    for member in members {
        let Some(name) = member.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some((member_kind, member_signature)) = impl_item_signature(name, member, index, false)
        else {
            continue;
        };
        append_api_toc_member(
            markdown,
            api_toc,
            heading_count,
            doc_links,
            member_kind,
            name,
            &member_signature,
            member.get("docs").and_then(Value::as_str),
            2,
            3,
            Some(member),
            true,
            1,
        );
    }
}

fn append_implementations(
    markdown: &mut String,
    item: &Value,
    index: &serde_json::Map<String, Value>,
    heading_count: &mut usize,
    api_toc: &mut Vec<ApiTocItem>,
    doc_links: &mut Vec<DocLinkGroup>,
) {
    let Some(inner) = item.get("inner").and_then(Value::as_object) else {
        return;
    };
    let Some((kind, data)) = inner
        .iter()
        .find(|(kind, _)| matches!(kind.as_str(), "struct" | "enum" | "union"))
    else {
        return;
    };
    let Some(impls) = data.get("impls").and_then(Value::as_array) else {
        return;
    };
    let name = item.get("name").and_then(Value::as_str).unwrap_or("Self");
    let data = inner.get(kind);
    let self_type = format!(
        "{name}{}",
        signature::generic_params(data.and_then(|data| data.get("generics")))
    );
    let mut inherent = Vec::new();
    let mut traits = Vec::new();
    let mut auto_traits = Vec::new();
    let mut blanket = Vec::new();
    for impl_id in impls {
        let Some(impl_item) = signature::get_item(index, impl_id) else {
            continue;
        };
        let Some(implementation) = impl_item.get("inner").and_then(|inner| inner.get("impl"))
        else {
            continue;
        };
        if implementation
            .get("blanket_impl")
            .is_some_and(|value| !value.is_null())
        {
            blanket.push((impl_item, implementation));
        } else if implementation.get("is_synthetic").and_then(Value::as_bool) == Some(true) {
            auto_traits.push((impl_item, implementation));
        } else if implementation
            .get("trait")
            .is_some_and(|value| !value.is_null())
        {
            traits.push((impl_item, implementation));
        } else {
            inherent.push((impl_item, implementation));
        }
    }

    append_impl_group(
        markdown,
        "Implementations",
        &inherent,
        &self_type,
        index,
        true,
        heading_count,
        api_toc,
        doc_links,
    );
    append_impl_group(
        markdown,
        "Trait Implementations",
        &traits,
        &self_type,
        index,
        false,
        heading_count,
        api_toc,
        doc_links,
    );
    append_impl_group(
        markdown,
        "Auto Trait Implementations",
        &auto_traits,
        &self_type,
        index,
        false,
        heading_count,
        api_toc,
        doc_links,
    );
    append_impl_group(
        markdown,
        "Blanket Implementations",
        &blanket,
        &self_type,
        index,
        false,
        heading_count,
        api_toc,
        doc_links,
    );
}

fn append_impl_group(
    markdown: &mut String,
    heading: &str,
    implementations: &[(&Value, &Value)],
    self_type: &str,
    index: &serde_json::Map<String, Value>,
    inherent: bool,
    heading_count: &mut usize,
    api_toc: &mut Vec<ApiTocItem>,
    doc_links: &mut Vec<DocLinkGroup>,
) {
    if implementations.is_empty() {
        return;
    }

    let mut implementations = implementations.to_vec();
    implementations.sort_by(|left, right| {
        implementation_signature(left.1, self_type, false)
            .cmp(&implementation_signature(right.1, self_type, false))
    });
    markdown.push_str(&format!("## {heading}\n\n"));
    *heading_count += 1;
    for (impl_item, implementation) in implementations {
        let signature = implementation_signature(implementation, self_type, true);
        let trait_name = implementation
            .get("trait")
            .filter(|value| !value.is_null())
            .map(signature::trait_name);
        let trait_anchor = trait_name.as_ref().map(|name| {
            member_anchor(
                "trait",
                name,
                item_id(impl_item).unwrap_or(api_toc.len() as u64),
            )
        });
        if let (Some(trait_name), Some(trait_anchor)) = (&trait_name, &trait_anchor) {
            api_toc.push(ApiTocItem {
                title: trait_name.clone(),
                url: format!("#{trait_anchor}"),
                depth: 3,
                insert_at: *heading_count,
                shift_next: 0,
                shift_depth: 0,
            });
        }
        let mut methods = implementation
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|method_id| signature::get_item(index, method_id))
            .filter(|method| {
                !inherent || method.get("visibility").and_then(Value::as_str) == Some("public")
            })
            .collect::<Vec<_>>();
        methods.sort_by(|left, right| {
            left.get("name")
                .and_then(Value::as_str)
                .cmp(&right.get("name").and_then(Value::as_str))
        });
        methods.dedup_by(|left, right| left.get("id") == right.get("id"));

        let members = methods
            .into_iter()
            .filter_map(|method| {
                let name = method.get("name").and_then(Value::as_str)?;
                let (member_kind, signature) = impl_item_signature(name, method, index, inherent)?;
                Some((
                    member_kind,
                    name.to_owned(),
                    signature,
                    method.get("docs").and_then(Value::as_str),
                    method,
                ))
            })
            .collect::<Vec<_>>();

        if members.is_empty() {
            if let Some(trait_anchor) = trait_anchor.as_deref() {
                append_api_member_without_icon_with_id(
                    markdown,
                    "implementation",
                    &signature,
                    None,
                    2,
                    Some(trait_anchor),
                    None,
                    doc_links,
                );
            } else {
                append_api_member_without_icon(
                    markdown,
                    "implementation",
                    &signature,
                    None,
                    2,
                    None,
                    doc_links,
                );
            }
            continue;
        }

        markdown.push_str(&format!(
            "<ApiImpl signature={}{}>\n\n",
            mdx_string(&signature),
            trait_anchor
                .as_deref()
                .map(|anchor| format!(" id={}", mdx_string(anchor)))
                .unwrap_or_default()
        ));
        for (member_kind, name, signature, docs, method) in members {
            append_api_toc_member(
                markdown,
                api_toc,
                heading_count,
                doc_links,
                member_kind,
                &name,
                &signature,
                docs,
                2,
                if inherent { 3 } else { 4 },
                Some(method),
                false,
                if inherent { 1 } else { 2 },
            );
        }
        markdown.push_str("</ApiImpl>\n\n");
    }
}

fn item_id(item: &Value) -> Option<u64> {
    item.get("id").and_then(Value::as_u64)
}

fn impl_item_signature(
    name: &str,
    item: &Value,
    index: &serde_json::Map<String, Value>,
    inherent: bool,
) -> Option<(&'static str, String)> {
    let inner = item.get("inner")?.as_object()?;
    if inner.contains_key("function") {
        return Some((
            "method",
            signature::function_signature(name, item, inherent),
        ));
    }
    for kind in ["type_alias", "constant", "static"] {
        if inner.contains_key(kind) {
            return Some(("associated", signature::signature(kind, name, item, index)));
        }
    }
    if inner.contains_key("assoc_type") {
        return Some(("associated", format!("type {name};")));
    }
    if inner.contains_key("assoc_const") {
        return Some(("associated", format!("const {name}: _;")));
    }
    None
}

fn append_api_toc_member(
    markdown: &mut String,
    api_toc: &mut Vec<ApiTocItem>,
    heading_count: &mut usize,
    doc_links: &mut Vec<DocLinkGroup>,
    kind: &str,
    name: &str,
    member_signature: &str,
    docs: Option<&str>,
    parent_heading_level: usize,
    depth: usize,
    item: Option<&Value>,
    show_icon: bool,
    shift_depth: usize,
) {
    let id = member_anchor(
        "member",
        name,
        item.and_then(item_id).unwrap_or(api_toc.len() as u64),
    );
    let doc_heading_count = docs.map(text::heading_count).unwrap_or_default();
    api_toc.push(ApiTocItem {
        title: name.to_owned(),
        url: format!("#{id}"),
        depth,
        insert_at: *heading_count,
        shift_next: doc_heading_count,
        shift_depth,
    });
    *heading_count += doc_heading_count;
    append_api_member_with_icon(
        markdown,
        kind,
        member_signature,
        docs,
        show_icon,
        parent_heading_level,
        Some(&id),
        item,
        doc_links,
    );
}

fn implementation_signature(implementation: &Value, self_type: &str, link_trait: bool) -> String {
    let mut signature = String::new();
    if implementation.get("is_unsafe").and_then(Value::as_bool) == Some(true) {
        signature.push_str("unsafe ");
    }
    signature.push_str("impl");
    signature.push_str(&signature::generic_params(implementation.get("generics")));
    signature.push(' ');

    if let Some(trait_type) = implementation.get("trait").filter(|value| !value.is_null()) {
        if implementation.get("is_negative").and_then(Value::as_bool) == Some(true) {
            signature.push('!');
        }
        let name = if link_trait {
            signature::linked_trait_name(trait_type)
        } else {
            signature::trait_name(trait_type)
        };
        signature.push_str(&name);
        signature.push_str(" for ");
    }
    signature.push_str(self_type);
    signature.push_str(&signature::where_clause(implementation.get("generics")));
    signature
}

fn append_api_member_without_icon(
    markdown: &mut String,
    kind: &str,
    signature: &str,
    docs: Option<&str>,
    parent_heading_level: usize,
    item: Option<&Value>,
    doc_links: &mut Vec<DocLinkGroup>,
) {
    append_api_member_with_icon(
        markdown,
        kind,
        signature,
        docs,
        false,
        parent_heading_level,
        None,
        item,
        doc_links,
    );
}

fn append_api_member_without_icon_with_id(
    markdown: &mut String,
    kind: &str,
    signature: &str,
    docs: Option<&str>,
    parent_heading_level: usize,
    id: Option<&str>,
    item: Option<&Value>,
    doc_links: &mut Vec<DocLinkGroup>,
) {
    append_api_member_with_icon(
        markdown,
        kind,
        signature,
        docs,
        false,
        parent_heading_level,
        id,
        item,
        doc_links,
    );
}

fn append_api_member_with_icon(
    markdown: &mut String,
    kind: &str,
    signature: &str,
    docs: Option<&str>,
    show_icon: bool,
    parent_heading_level: usize,
    id: Option<&str>,
    item: Option<&Value>,
    doc_links: &mut Vec<DocLinkGroup>,
) {
    let kind = mdx_string(kind);
    let signature = mdx_string(signature);
    let icon_attribute = if show_icon { "" } else { " showIcon={false}" };
    let id_attribute = id
        .map(|id| format!(" id={}", mdx_string(id)))
        .unwrap_or_default();
    if let Some(docs) = docs.filter(|docs| !docs.trim().is_empty()) {
        let rendered_docs = text::shift_doc_headings(docs.trim(), parent_heading_level);
        if let Some(item) = item {
            doc_links.push(doc_link_group(&rendered_docs, item));
        }
        markdown.push_str(&format!(
            "<ApiMember kind={kind} signature={signature}{icon_attribute}{id_attribute}>\n\n"
        ));
        markdown.push_str(&rendered_docs);
        markdown.push_str("\n\n</ApiMember>\n\n");
    } else {
        markdown.push_str(&format!(
            "<ApiMember kind={kind} signature={signature}{icon_attribute}{id_attribute} />\n\n"
        ));
    }
}

fn mdx_string(value: &str) -> String {
    format!("{{{}}}", text::yaml_quote(value))
}
