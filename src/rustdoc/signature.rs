mod syntax;

use serde_json::{Map, Value};

use syntax::type_name;
pub(super) use syntax::{
    field_type, function_signature, generic_params, get_item, linked_trait_name, trait_name,
    visibility, where_clause,
};

pub(super) fn signature(
    kind: &str,
    name: &str,
    item: &Value,
    index: &Map<String, Value>,
) -> String {
    let visibility = visibility(item);
    let inner = item.get("inner").and_then(Value::as_object);
    let data = inner
        .and_then(|inner| inner.get(kind))
        .or_else(|| inner.and_then(|inner| inner.values().next()));

    match kind {
        "module" => format!("{visibility}mod {name};"),
        "reexport" => reexport_signature(&visibility, name, data),
        "function" => function_signature(name, item, true),
        "struct" => struct_signature(&visibility, name, data, index),
        "enum" => format!(
            "{visibility}enum {name}{} {{ ... }}",
            generic_params(data.and_then(|value| value.get("generics")))
        ),
        "trait" => trait_signature(&visibility, name, data, false),
        "trait_alias" => trait_signature(&visibility, name, data, true),
        "type_alias" => type_alias_signature(&visibility, name, data),
        "constant" => constant_signature(&visibility, name, data),
        "static" => static_signature(&visibility, name, data),
        "union" => format!("{visibility}union {name} {{ ... }}"),
        "macro" | "proc_macro" | "proc_attribute" | "proc_derive" => {
            format!("{visibility}macro {name} {{ ... }}")
        }
        _ => format!("{visibility}{kind} {name}"),
    }
}

fn reexport_signature(visibility: &str, name: &str, data: Option<&Value>) -> String {
    let source = data
        .and_then(|value| value.get("source"))
        .and_then(Value::as_str)
        .unwrap_or(name);
    let source_name = source.rsplit("::").next().unwrap_or(source);

    if source_name == name {
        format!("{visibility}use {source};")
    } else {
        format!("{visibility}use {source} as {name};")
    }
}

fn struct_signature(
    visibility: &str,
    name: &str,
    data: Option<&Value>,
    index: &Map<String, Value>,
) -> String {
    let generics = generic_params(data.and_then(|value| value.get("generics")));
    let shape = data.and_then(|value| value.get("kind"));

    if shape.is_some_and(|shape| shape.get("unit").is_some()) {
        return format!("{visibility}struct {name}{generics};");
    }

    if let Some(tuple) = shape
        .and_then(|shape| shape.get("tuple"))
        .and_then(Value::as_array)
    {
        let fields = tuple
            .iter()
            .map(|id| {
                get_item(index, id)
                    .map(field_type)
                    .unwrap_or_else(|| "_".to_owned())
            })
            .collect::<Vec<_>>()
            .join(", ");
        return format!("{visibility}struct {name}{generics}({fields});");
    }

    format!("{visibility}struct {name}{generics} {{ ... }}")
}

fn trait_signature(visibility: &str, name: &str, data: Option<&Value>, is_alias: bool) -> String {
    let generics = data.and_then(|value| value.get("generics"));
    let where_clause = where_clause(generics);
    let body = match (is_alias, where_clause.is_empty()) {
        (false, true) => " { ... }".to_owned(),
        (false, false) => format!("{where_clause}\n{{ ... }}"),
        (true, true) => " = ...;".to_owned(),
        (true, false) => format!("{where_clause}\n= ...;"),
    };

    format!("{visibility}trait {name}{}{body}", generic_params(generics))
}

fn type_alias_signature(visibility: &str, name: &str, data: Option<&Value>) -> String {
    let generics = generic_params(data.and_then(|value| value.get("generics")));
    let ty = item_type(data);
    format!("{visibility}type {name}{generics} = {ty};")
}

fn constant_signature(visibility: &str, name: &str, data: Option<&Value>) -> String {
    let ty = item_type(data);
    let constant = data.and_then(|value| value.get("const"));
    let value = constant
        .and_then(|constant| constant.get("value"))
        .and_then(Value::as_str)
        .or_else(|| {
            constant
                .and_then(|constant| constant.get("expr"))
                .and_then(Value::as_str)
        })
        .unwrap_or("…");

    format!("{visibility}const {name}: {ty} = {value};")
}

fn static_signature(visibility: &str, name: &str, data: Option<&Value>) -> String {
    let ty = item_type(data);
    let mutable = data
        .and_then(|value| value.get("is_mutable"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mutable = if mutable { "mut " } else { "" };

    format!("{visibility}static {mutable}{name}: {ty};")
}

fn item_type(data: Option<&Value>) -> String {
    data.and_then(|value| value.get("type"))
        .map(type_name)
        .unwrap_or_else(|| "_".to_owned())
}

pub(super) fn variant_signature(name: &str, item: &Value, index: &Map<String, Value>) -> String {
    let shape = item
        .get("inner")
        .and_then(|inner| inner.get("variant"))
        .and_then(|data| data.get("kind"));
    if let Some(tuple) = shape
        .and_then(|shape| shape.get("tuple"))
        .and_then(Value::as_array)
    {
        let fields = tuple
            .iter()
            .map(|id| {
                get_item(index, id)
                    .map(field_type)
                    .unwrap_or_else(|| "_".to_owned())
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("{name}({fields})")
    } else if let Some(structure) = shape.and_then(|shape| shape.get("struct")) {
        let fields = structure
            .get("fields")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|id| get_item(index, id))
            .map(|field| {
                let field_name = field.get("name").and_then(Value::as_str).unwrap_or("field");
                format!("{field_name}: {}", field_type(field))
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("{name} {{ {fields} }}")
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn renders_unit_structs() {
        let item = json!({
            "visibility": "public",
            "inner": {
                "struct": {
                    "kind": { "unit": null },
                    "generics": { "params": [], "where_predicates": [] }
                }
            }
        });

        assert_eq!(
            signature("struct", "Marker", &item, &Map::new()),
            "pub struct Marker;"
        );
    }

    #[test]
    fn renders_function_headers_in_rust_order() {
        let item = json!({
            "visibility": "public",
            "inner": {
                "function": {
                    "sig": {
                        "inputs": [["value", { "generic": "T" }]],
                        "output": { "generic": "T" },
                        "is_c_variadic": false
                    },
                    "header": {
                        "is_const": true,
                        "is_async": true,
                        "is_unsafe": true,
                        "abi": "C"
                    },
                    "generics": {
                        "params": [{ "name": "T", "kind": { "type": { "bounds": [] } } }],
                        "where_predicates": []
                    }
                }
            }
        });

        assert_eq!(
            function_signature("convert", &item, true),
            "pub const async unsafe extern \"C\" fn convert<T>(value: T) -> T"
        );
    }

    #[test]
    fn renders_renamed_reexports() {
        let item = json!({
            "visibility": "public",
            "inner": { "use": { "source": "source::Original" } }
        });

        assert_eq!(
            signature("reexport", "Alias", &item, &Map::new()),
            "pub use source::Original as Alias;"
        );
    }
}
