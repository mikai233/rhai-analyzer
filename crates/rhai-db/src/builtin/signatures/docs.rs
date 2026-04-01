use rhai_hir::{FunctionTypeRef, TypeRef};

pub(crate) fn builtin_global_docs(
    name: &str,
    signatures: &[FunctionTypeRef],
    summary: &str,
    examples: &[&str],
    reference_url: &str,
) -> String {
    render_callable_docs(
        summary,
        global_usage_lines(name, signatures),
        global_example_lines(name, signatures, examples),
        reference_url,
    )
}

pub(crate) fn builtin_method_docs(
    receiver_type: &str,
    name: &str,
    signatures: &[FunctionTypeRef],
    summary: &str,
    examples: &[&str],
    reference_url: &str,
) -> String {
    render_callable_docs(
        summary,
        method_usage_lines(receiver_type, name, signatures),
        method_example_lines(receiver_type, name, signatures, examples),
        reference_url,
    )
}

pub(crate) fn builtin_type_docs(
    type_name: &str,
    summary: &str,
    examples: &[&str],
    reference_url: &str,
) -> String {
    let mut sections = vec![summary.trim().to_owned()];

    if !examples.is_empty() {
        sections.push(format!(
            "## Examples\n```rhai\n{}\n```",
            examples.join("\n")
        ));
    }

    sections.push(format!(
        "## Official Rhai Reference\n[Rhai Book]({reference_url}) · `{type_name}`"
    ));

    sections.join("\n\n")
}

pub(crate) fn builtin_topic_docs(
    summary: &str,
    usage_lines: &[&str],
    examples: &[&str],
    reference_url: &str,
) -> String {
    let mut sections = vec![summary.trim().to_owned()];

    if !usage_lines.is_empty() {
        sections.push(format!(
            "## Usage\n```rhai\n{}\n```",
            usage_lines.join("\n")
        ));
    }

    if !examples.is_empty() {
        sections.push(format!(
            "## Examples\n```rhai\n{}\n```",
            examples.join("\n")
        ));
    }

    sections.push(format!(
        "## Official Rhai Reference\n[Rhai Book]({reference_url})"
    ));

    sections.join("\n\n")
}

fn render_callable_docs(
    summary: &str,
    usage_lines: Vec<String>,
    example_lines: Vec<String>,
    reference_url: &str,
) -> String {
    let mut sections = vec![summary.trim().to_owned()];

    if !usage_lines.is_empty() {
        sections.push(format!(
            "## Usage\n```rhai\n{}\n```",
            usage_lines.join("\n")
        ));
    }

    if !example_lines.is_empty() {
        sections.push(format!(
            "## Examples\n```rhai\n{}\n```",
            example_lines.join("\n")
        ));
    }

    sections.push(format!(
        "## Official Rhai Reference\n[Rhai Book]({reference_url})"
    ));

    sections.join("\n\n")
}

fn global_usage_lines(name: &str, signatures: &[FunctionTypeRef]) -> Vec<String> {
    signatures
        .iter()
        .map(|signature| {
            let args = signature
                .params
                .iter()
                .map(example_value_for_type)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}({args})")
        })
        .collect()
}

fn method_usage_lines(
    receiver_type: &str,
    name: &str,
    signatures: &[FunctionTypeRef],
) -> Vec<String> {
    let receiver = receiver_example(receiver_type);

    signatures
        .iter()
        .map(|signature| {
            let args = signature
                .params
                .iter()
                .map(example_value_for_type)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{receiver}.{name}({args})")
        })
        .collect()
}

fn global_example_lines(
    name: &str,
    signatures: &[FunctionTypeRef],
    examples: &[&str],
) -> Vec<String> {
    if !examples.is_empty() {
        return examples
            .iter()
            .map(|example| (*example).to_owned())
            .collect::<Vec<_>>();
    }

    signatures
        .iter()
        .flat_map(|signature| {
            let call = invocation(name, &signature.params, None);
            vec![
                assignment_or_statement(call, signature.ret.as_ref()),
                comment_for_result(signature.ret.as_ref(), None),
            ]
        })
        .collect()
}

fn method_example_lines(
    receiver_type: &str,
    name: &str,
    signatures: &[FunctionTypeRef],
    examples: &[&str],
) -> Vec<String> {
    if !examples.is_empty() {
        return examples
            .iter()
            .map(|example| (*example).to_owned())
            .collect::<Vec<_>>();
    }

    let receiver = receiver_example(receiver_type);

    signatures
        .iter()
        .enumerate()
        .flat_map(|(index, signature)| {
            let binding = if index == 0 {
                vec![format!("let value = {receiver};")]
            } else {
                Vec::new()
            };
            let call = invocation(name, &signature.params, Some("value"));
            binding
                .into_iter()
                .chain([
                    assignment_or_statement(call, signature.ret.as_ref()),
                    comment_for_result(signature.ret.as_ref(), Some(receiver_type)),
                ])
                .collect::<Vec<_>>()
        })
        .collect()
}

fn invocation(name: &str, params: &[TypeRef], receiver: Option<&str>) -> String {
    let args = params
        .iter()
        .map(example_value_for_type)
        .collect::<Vec<_>>()
        .join(", ");

    match receiver {
        Some(receiver) => format!("{receiver}.{name}({args})"),
        None => format!("{name}({args})"),
    }
}

fn assignment_or_statement(call: String, ret: &TypeRef) -> String {
    if matches!(ret, TypeRef::Unit) {
        format!("{call};")
    } else {
        format!("let result = {call};")
    }
}

fn comment_for_result(ret: &TypeRef, receiver_type: Option<&str>) -> String {
    match (ret, receiver_type) {
        (TypeRef::Unit, Some(_)) => "// result: mutates the receiver in place".to_owned(),
        (TypeRef::Unit, None) => "// returns: ()".to_owned(),
        _ => format!("// returns: {}", result_type_label(ret)),
    }
}

fn result_type_label(ret: &TypeRef) -> String {
    match ret {
        TypeRef::Unknown => "unknown".to_owned(),
        TypeRef::Any => "any".to_owned(),
        TypeRef::Never => "never".to_owned(),
        TypeRef::Dynamic => "Dynamic".to_owned(),
        TypeRef::Bool => "bool".to_owned(),
        TypeRef::Int => "int".to_owned(),
        TypeRef::Float => "float".to_owned(),
        TypeRef::Decimal => "decimal".to_owned(),
        TypeRef::String => "string".to_owned(),
        TypeRef::Char => "char".to_owned(),
        TypeRef::Blob => "blob".to_owned(),
        TypeRef::Timestamp => "timestamp".to_owned(),
        TypeRef::FnPtr => "Fn".to_owned(),
        TypeRef::Unit => "()".to_owned(),
        TypeRef::Range => "range".to_owned(),
        TypeRef::RangeInclusive => "range=".to_owned(),
        TypeRef::Named(name) => name.clone(),
        TypeRef::Applied { name, .. } => name.clone(),
        TypeRef::Object(_) => "map".to_owned(),
        TypeRef::Array(inner) => format!("array<{}>", result_type_label(inner)),
        TypeRef::Map(key, value) => {
            format!(
                "map<{}, {}>",
                result_type_label(key),
                result_type_label(value)
            )
        }
        TypeRef::Nullable(inner) => format!("{}?", result_type_label(inner)),
        TypeRef::Union(members) => members
            .iter()
            .map(result_type_label)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeRef::Ambiguous(members) => format!(
            "ambiguous<{}>",
            members
                .iter()
                .map(result_type_label)
                .collect::<Vec<_>>()
                .join(" | ")
        ),
        TypeRef::Function(signature) => format!(
            "fun({}) -> {}",
            signature
                .params
                .iter()
                .map(result_type_label)
                .collect::<Vec<_>>()
                .join(", "),
            result_type_label(signature.ret.as_ref())
        ),
    }
}

fn receiver_example(receiver_type: &str) -> String {
    match receiver_type {
        "string" => "\"hello\"".to_owned(),
        "array" => "[1, 2, 3]".to_owned(),
        "map" => "#{ name: \"Ada\", active: true }".to_owned(),
        "blob" => "blob(4, 0)".to_owned(),
        "int" => "42".to_owned(),
        "float" => "3.14".to_owned(),
        "char" => "'x'".to_owned(),
        "timestamp" => "timestamp()".to_owned(),
        "range" => "0..10".to_owned(),
        "range=" => "0..=10".to_owned(),
        _ => "value".to_owned(),
    }
}

fn example_value_for_type(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic => "value".to_owned(),
        TypeRef::Never => "()".to_owned(),
        TypeRef::Bool => "true".to_owned(),
        TypeRef::Int => "1".to_owned(),
        TypeRef::Float | TypeRef::Decimal => "1.5".to_owned(),
        TypeRef::String => "\"text\"".to_owned(),
        TypeRef::Char => "'x'".to_owned(),
        TypeRef::Blob => "blob(2, 0)".to_owned(),
        TypeRef::Timestamp => "timestamp()".to_owned(),
        TypeRef::FnPtr => "Fn(\"handler\")".to_owned(),
        TypeRef::Unit => "()".to_owned(),
        TypeRef::Range => "0..3".to_owned(),
        TypeRef::RangeInclusive => "0..=3".to_owned(),
        TypeRef::Named(name) => format!("{name}_value"),
        TypeRef::Applied { name, .. } => format!("{name}_value"),
        TypeRef::Object(_) => "#{ name: \"Ada\" }".to_owned(),
        TypeRef::Array(inner) => format!("[{}]", example_value_for_type(inner)),
        TypeRef::Map(_, value) => format!("#{{ key: {} }}", example_value_for_type(value)),
        TypeRef::Nullable(inner) => example_value_for_type(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => members
            .iter()
            .find(|member| !matches!(member, TypeRef::Unit))
            .map(example_value_for_type)
            .unwrap_or_else(|| "()".to_owned()),
        TypeRef::Function(_) => "Fn(\"handler\")".to_owned(),
    }
}
