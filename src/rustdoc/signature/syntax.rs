use serde_json::{Map, Value};

use super::super::{SIGNATURE_LINK_END, SIGNATURE_LINK_SEPARATOR, SIGNATURE_LINK_START};

fn link_reference(id: &str, label: &str) -> String {
    format!("{SIGNATURE_LINK_START}{id}{SIGNATURE_LINK_SEPARATOR}{label}{SIGNATURE_LINK_END}")
}

fn linkable_name(value: &Value, name: &str) -> String {
    value
        .get("id")
        .and_then(Value::as_u64)
        .map(|id| link_reference(&id.to_string(), name))
        .unwrap_or_else(|| name.to_owned())
}

fn without_link_references(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some(relative_start) = value[cursor..].find(SIGNATURE_LINK_START) {
        let start = cursor + relative_start;
        output.push_str(&value[cursor..start]);
        let key_start = start + SIGNATURE_LINK_START.len_utf8();
        let Some(relative_separator) = value[key_start..].find(SIGNATURE_LINK_SEPARATOR) else {
            output.push(SIGNATURE_LINK_START);
            cursor = key_start;
            continue;
        };
        let label_start = key_start + relative_separator + SIGNATURE_LINK_SEPARATOR.len_utf8();
        let Some(relative_end) = value[label_start..].find(SIGNATURE_LINK_END) else {
            output.push(SIGNATURE_LINK_START);
            cursor = key_start;
            continue;
        };
        let end = label_start + relative_end;
        output.push_str(&value[label_start..end]);
        cursor = end + SIGNATURE_LINK_END.len_utf8();
    }
    output.push_str(&value[cursor..]);
    output
}

pub(crate) fn function_signature(name: &str, item: &Value, visibility_prefix: bool) -> String {
    let Some(function) = item.get("inner").and_then(|inner| inner.get("function")) else {
        return format!("fn {name}()");
    };

    let function_type = function.get("sig");
    let inputs = function_inputs(function_type);
    let output = function_output(function_type);
    let modifiers = function_modifiers(function.get("header"));
    let abi = function_abi(function.get("header"));
    let visibility = if visibility_prefix {
        visibility(item)
    } else {
        String::new()
    };
    let generics = function.get("generics");

    format!(
        "{visibility}{modifiers}{abi}fn {name}{}({inputs}){output}{}",
        generic_params(generics),
        where_clause(generics)
    )
}

fn function_inputs(signature: Option<&Value>) -> String {
    let mut inputs = signature
        .and_then(|signature| signature.get("inputs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|input| {
            let name = input.get(0).and_then(Value::as_str).unwrap_or("arg");
            let input_type = input.get(1).unwrap_or(&Value::Null);
            if name == "self" {
                return receiver_name(input_type);
            }
            let ty = type_name(input_type);
            format!("{name}: {ty}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    if signature
        .and_then(|signature| signature.get("is_c_variadic"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        if !inputs.is_empty() {
            inputs.push_str(", ");
        }
        inputs.push_str("...");
    }

    inputs
}

fn function_output(signature: Option<&Value>) -> String {
    signature
        .and_then(|signature| signature.get("output"))
        .filter(|value| !value.is_null())
        .map(|value| format!(" -> {}", type_name(value)))
        .unwrap_or_default()
}

fn function_modifiers(header: Option<&Value>) -> String {
    let mut modifiers = Vec::new();

    if header_flag(header, "is_const") {
        modifiers.push("const");
    }
    if header_flag(header, "is_async") {
        modifiers.push("async");
    }
    if header_flag(header, "is_unsafe") {
        modifiers.push("unsafe");
    }

    if modifiers.is_empty() {
        String::new()
    } else {
        format!("{} ", modifiers.join(" "))
    }
}

fn header_flag(header: Option<&Value>, name: &str) -> bool {
    header
        .and_then(|header| header.get(name))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn function_abi(header: Option<&Value>) -> String {
    header
        .and_then(|header| header.get("abi"))
        .and_then(Value::as_str)
        .filter(|abi| *abi != "Rust")
        .map(|abi| format!("extern \"{abi}\" "))
        .unwrap_or_default()
}

fn receiver_name(value: &Value) -> String {
    if let Some(reference) = value.get("borrowed_ref") {
        let lifetime = reference
            .get("lifetime")
            .and_then(Value::as_str)
            .map(|value| format!("{value} "))
            .unwrap_or_default();
        let mutable = reference
            .get("is_mutable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        return format!("&{lifetime}{}self", if mutable { "mut " } else { "" });
    }
    if value.get("generic").and_then(Value::as_str) == Some("Self") {
        return "self".to_owned();
    }
    format!("self: {}", type_name(value))
}

pub(crate) fn generic_params(generics: Option<&Value>) -> String {
    let params = generics
        .and_then(|generics| generics.get("params"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(generic_param)
        .collect::<Vec<_>>();
    if params.is_empty() {
        String::new()
    } else {
        format!("<{}>", params.join(", "))
    }
}

fn generic_param(param: &Value) -> Option<String> {
    let name = param.get("name").and_then(Value::as_str)?;
    let kind = param.get("kind")?;

    if let Some(type_param) = kind.get("type") {
        let mut output = name.to_owned();
        let bounds = type_param
            .get("bounds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(generic_bound)
            .collect::<Vec<_>>();
        if !bounds.is_empty() {
            output.push_str(&format!(": {}", bounds.join(" + ")));
        }
        if let Some(default) = type_param.get("default").filter(|value| !value.is_null()) {
            output.push_str(&format!(" = {}", type_name(default)));
        }
        return Some(output);
    }

    if let Some(const_param) = kind.get("const") {
        let type_name = const_param
            .get("type")
            .map(type_name)
            .unwrap_or_else(|| "_".to_owned());
        let default = const_param
            .get("default")
            .and_then(Value::as_str)
            .map(|value| format!(" = {value}"))
            .unwrap_or_default();
        return Some(format!("{name}: {type_name}{default}"));
    }

    Some(name.to_owned())
}

pub(crate) fn where_clause(generics: Option<&Value>) -> String {
    let predicates = generics
        .and_then(|generics| generics.get("where_predicates"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(where_predicate)
        .collect::<Vec<_>>();
    if predicates.is_empty() {
        String::new()
    } else {
        format!("\nwhere\n    {},", predicates.join(",\n    "))
    }
}

fn where_predicate(predicate: &Value) -> Option<String> {
    if let Some(predicate) = predicate.get("bound_predicate") {
        let type_name = predicate.get("type").map(type_name)?;
        let bounds = predicate
            .get("bounds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(generic_bound)
            .collect::<Vec<_>>();
        if bounds.is_empty() {
            return None;
        }
        let generic_params = predicate
            .get("generic_params")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|param| param.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        let type_name = if generic_params.is_empty() {
            type_name
        } else {
            format!("for<{}> {type_name}", generic_params.join(", "))
        };
        return Some(format!("{type_name}: {}", bounds.join(" + ")));
    }

    if let Some(predicate) = predicate.get("lifetime_predicate") {
        let lifetime = predicate.get("lifetime").and_then(Value::as_str)?;
        let bounds = predicate
            .get("bounds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|bound| {
                bound
                    .as_str()
                    .or_else(|| bound.get("outlives").and_then(Value::as_str))
            })
            .collect::<Vec<_>>();
        if bounds.is_empty() {
            return None;
        }
        return Some(format!("{lifetime}: {}", bounds.join(" + ")));
    }

    if let Some(predicate) = predicate.get("eq_predicate") {
        let left = predicate.get("lhs").map(type_name)?;
        let right = predicate.get("rhs").map(type_name)?;
        return Some(format!("{left} = {right}"));
    }

    None
}

fn generic_bound(bound: &Value) -> Option<String> {
    if let Some(bound) = bound.get("trait_bound") {
        let trait_name = bound.get("trait").map(linked_trait_name)?;
        let modifier = match bound.get("modifier").and_then(Value::as_str) {
            Some("maybe") => "?",
            Some("maybe_const") => "~const ",
            _ => "",
        };
        let generic_params = bound
            .get("generic_params")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|param| param.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        let trait_name = if generic_params.is_empty() {
            format!("{modifier}{trait_name}")
        } else {
            format!("for<{}> {modifier}{trait_name}", generic_params.join(", "))
        };
        return Some(trait_name);
    }

    bound
        .get("outlives")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn generic_arguments(args: Option<&Value>) -> Vec<String> {
    let Some(args) = args.and_then(|args| args.get("angle_bracketed")) else {
        return Vec::new();
    };
    let arguments = args
        .get("args")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(generic_argument);
    let constraints = args
        .get("constraints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(generic_constraint);
    arguments.chain(constraints).collect()
}

fn generic_argument(argument: &Value) -> Option<String> {
    if let Some(ty) = argument.get("type") {
        return Some(type_name(ty));
    }
    if let Some(lifetime) = argument.get("lifetime").and_then(Value::as_str) {
        return Some(lifetime.to_owned());
    }
    argument
        .get("const")
        .map(|value| value.as_str().unwrap_or("_").to_owned())
}

fn generic_constraint(constraint: &Value) -> Option<String> {
    let name = constraint.get("name").and_then(Value::as_str)?;
    let binding = constraint.get("binding")?;
    if let Some(equality) = binding.get("equality") {
        if let Some(value) = equality.get("type") {
            return Some(format!("{name} = {}", type_name(value)));
        }
        if let Some(value) = equality.get("const").and_then(Value::as_str) {
            return Some(format!("{name} = {value}"));
        }
        return None;
    }
    let bounds = binding
        .get("constraint")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(generic_bound)
        .collect::<Vec<_>>();
    if bounds.is_empty() {
        None
    } else {
        Some(format!("{name}: {}", bounds.join(" + ")))
    }
}

pub(super) fn type_name(value: &Value) -> String {
    let Some(object) = value.as_object() else {
        return "_".to_owned();
    };

    if let Some(value) = object.get("primitive").and_then(Value::as_str) {
        return link_reference(&format!("primitive:{value}"), value);
    }

    if let Some(value) = object.get("generic").and_then(Value::as_str) {
        return value.to_owned();
    }

    if let Some(path) = object.get("resolved_path") {
        let name = path.get("path").and_then(Value::as_str).unwrap_or("_");
        let name = linkable_name(path, name);
        let args = generic_arguments(path.get("args"));
        return if args.is_empty() {
            name
        } else {
            format!("{name}<{}>", args.join(", "))
        };
    }

    if let Some(reference) = object.get("borrowed_ref") {
        let lifetime = reference
            .get("lifetime")
            .and_then(Value::as_str)
            .map(|value| format!("{value} "))
            .unwrap_or_default();
        let mutable = reference
            .get("is_mutable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let ty = reference
            .get("type")
            .map(type_name)
            .unwrap_or_else(|| "_".to_owned());
        return format!("&{lifetime}{}{ty}", if mutable { "mut " } else { "" });
    }

    if let Some(pointer) = object.get("raw_pointer") {
        let ty = pointer
            .get("type")
            .map(type_name)
            .unwrap_or_else(|| "_".to_owned());
        return format!(
            "*{}{ty}",
            if pointer.get("is_mutable").and_then(Value::as_bool) == Some(true) {
                "mut "
            } else {
                "const "
            }
        );
    }

    if let Some(slice) = object.get("slice") {
        return format!("[{}]", type_name(slice));
    }

    if let Some(array) = object.get("array") {
        let ty = array
            .get("type")
            .map(type_name)
            .unwrap_or_else(|| "_".to_owned());
        let len = array.get("len").and_then(Value::as_str).unwrap_or("_");
        return format!("[{ty}; {len}]");
    }

    if let Some(tuple) = object.get("tuple").and_then(Value::as_array) {
        let values = tuple.iter().map(type_name).collect::<Vec<_>>();
        let name = if values.len() == 1 {
            format!("({},)", values[0])
        } else {
            format!("({})", values.join(", "))
        };
        return name;
    }

    if let Some(path) = object.get("qualified_path") {
        let self_type = path
            .get("self_type")
            .map(type_name)
            .unwrap_or_else(|| "_".to_owned());
        let name = path.get("name").and_then(Value::as_str).unwrap_or("_");
        return format!("<{self_type}>::{name}");
    }

    "_".to_owned()
}

pub(crate) fn field_type(item: &Value) -> String {
    item.get("inner")
        .and_then(|inner| inner.get("struct_field"))
        .map(type_name)
        .unwrap_or_else(|| "_".to_owned())
}

pub(crate) fn get_item<'a>(index: &'a Map<String, Value>, id: &Value) -> Option<&'a Value> {
    let key = id.as_u64()?.to_string();
    index.get(&key)
}

pub(crate) fn visibility(item: &Value) -> String {
    match item.get("visibility").and_then(Value::as_str) {
        Some("public") => "pub ".to_owned(),
        Some("restricted") => "pub(crate) ".to_owned(),
        _ => String::new(),
    }
}

pub(crate) fn trait_name(value: &Value) -> String {
    let name = value
        .get("path")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("resolved_path")
                .and_then(|path| path.get("path"))
                .and_then(Value::as_str)
        })
        .unwrap_or("Trait");
    let args = generic_arguments(
        value
            .get("args")
            .or_else(|| value.get("resolved_path").and_then(|path| path.get("args"))),
    );
    let name = if args.is_empty() {
        name.to_owned()
    } else {
        format!("{name}<{}>", args.join(", "))
    };
    without_link_references(&name)
}

pub(crate) fn linked_trait_name(value: &Value) -> String {
    let path = value.get("resolved_path").unwrap_or(value);
    let name = path
        .get("path")
        .and_then(Value::as_str)
        .or_else(|| value.get("path").and_then(Value::as_str))
        .unwrap_or("Trait");
    let name = linkable_name(path, name);
    let args = generic_arguments(value.get("args").or_else(|| path.get("args")));
    if args.is_empty() {
        name
    } else {
        format!("{name}<{}>", args.join(", "))
    }
}
