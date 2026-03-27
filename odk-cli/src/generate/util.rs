//! Shared utilities for code generation targets.

use odk_schema::{
    CompiledType, FragmentDefinition, OperationKind, ScalarType, SelectionSet, Variable,
};

/// Map a CompiledType to a TypeScript type string.
pub fn type_to_ts(t: &CompiledType) -> String {
    match t {
        CompiledType::Scalar { name } => scalar_to_ts(name).to_string(),
        CompiledType::Array { items } => format!("{}[]", type_to_ts(items)),
        CompiledType::Ref { name } => name.clone(),
        CompiledType::Literal { value } => match value {
            odk_schema::LiteralValue::String(s) => format!("\"{}\"", s),
            odk_schema::LiteralValue::Number(n) => n.to_string(),
            odk_schema::LiteralValue::Bool(b) => b.to_string(),
        },
        CompiledType::Union { variants } => {
            let parts: Vec<String> = variants.iter().map(type_to_ts).collect();
            parts.join(" | ")
        }
        CompiledType::Object { fields } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|f| {
                    let opt = if f.optional == Some(true) { "?" } else { "" };
                    format!("{}{}: {}", f.name, opt, type_to_ts(&f.field_type))
                })
                .collect();
            format!("{{ {} }}", parts.join("; "))
        }
    }
}

fn scalar_to_ts(s: &ScalarType) -> &'static str {
    match s {
        ScalarType::String => "string",
        ScalarType::Int32 | ScalarType::Int64 => "number",
        ScalarType::Float32 | ScalarType::Float64 => "number",
        ScalarType::Boolean => "boolean",
        ScalarType::Date
        | ScalarType::Datetime
        | ScalarType::UtcDateTime
        | ScalarType::OffsetDateTime
        | ScalarType::PlainDate
        | ScalarType::PlainTime => "string",
        ScalarType::Duration => "string",
        ScalarType::Bytes => "string",
        ScalarType::Url => "string",
        ScalarType::Unknown => "unknown",
    }
}

/// Map a ScalarType to a sensible TypeScript mock default value.
pub fn scalar_to_mock(s: &ScalarType) -> &'static str {
    match s {
        ScalarType::String => "\"\"",
        ScalarType::Int32 | ScalarType::Int64 => "0",
        ScalarType::Float32 | ScalarType::Float64 => "0",
        ScalarType::Boolean => "false",
        ScalarType::Date | ScalarType::PlainDate => "\"2024-01-01\"",
        ScalarType::Datetime | ScalarType::UtcDateTime | ScalarType::OffsetDateTime => {
            "\"2024-01-01T00:00:00Z\""
        }
        ScalarType::PlainTime => "\"00:00:00\"",
        ScalarType::Duration => "\"PT0S\"",
        ScalarType::Bytes => "\"\"",
        ScalarType::Url => "\"https://example.com\"",
        ScalarType::Unknown => "null",
    }
}

/// Convert an operation name to a camelCase method name (lowercase first char).
/// e.g. "GetUser" -> "getUser", "CreatePost" -> "createPost"
pub fn to_method_name(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_lowercase().to_string() + chars.as_str(),
    }
}

/// Convert an operation name to a hook name.
/// e.g. "GetUser" -> "useGetUser"
pub fn to_hook_name(name: &str) -> String {
    format!("use{}", name)
}

/// Convert an operation name to a mock handler name.
/// e.g. "GetUser" -> "mockGetUser"
pub fn to_mock_name(name: &str) -> String {
    format!("mock{}", name)
}

/// Convert an operation name to a mock data factory name.
/// e.g. "GetUser" -> "mockGetUserData"
pub fn to_mock_data_name(name: &str) -> String {
    format!("mock{}Data", name)
}

/// Convert an operation name to a mock error handler name.
/// e.g. "GetUser" -> "mockGetUserError"
pub fn to_mock_error_name(name: &str) -> String {
    format!("mock{}Error", name)
}

/// Map GraphQL variable type to its GraphQL SDL string representation.
/// e.g. required String -> "String!", optional Int -> "Int"
pub fn variable_to_graphql(var: &Variable) -> String {
    let base = compiled_type_to_graphql(&var.variable_type);
    if var.required {
        format!("{}!", base)
    } else {
        base
    }
}

/// Map a CompiledType back to GraphQL SDL type notation.
fn compiled_type_to_graphql(t: &CompiledType) -> String {
    match t {
        CompiledType::Scalar { name } => scalar_to_graphql(name).to_string(),
        CompiledType::Array { items } => format!("[{}]", compiled_type_to_graphql(items)),
        CompiledType::Ref { name } => name.clone(),
        _ => "String".to_string(),
    }
}

fn scalar_to_graphql(s: &ScalarType) -> &'static str {
    match s {
        ScalarType::String => "String",
        ScalarType::Int32 | ScalarType::Int64 => "Int",
        ScalarType::Float32 | ScalarType::Float64 => "Float",
        ScalarType::Boolean => "Boolean",
        _ => "String",
    }
}

/// Reconstruct a GraphQL operation document string from the IR.
///
/// Produces the full operation text including variable definitions,
/// selection sets, fragment spreads, and inline fragments.
pub fn reconstruct_operation(
    name: &str,
    kind: &OperationKind,
    variables: &[Variable],
    selection: &Option<SelectionSet>,
    fragments: &[FragmentDefinition],
) -> String {
    let mut lines = Vec::new();

    // Operation keyword
    let keyword = match kind {
        OperationKind::Query => "query",
        OperationKind::Mutation => "mutation",
        OperationKind::Subscription => "subscription",
        _ => "query",
    };

    // Variable definitions
    let vars = if variables.is_empty() {
        String::new()
    } else {
        let var_strs: Vec<String> = variables
            .iter()
            .map(|v| format!("${}: {}", v.name, variable_to_graphql(v)))
            .collect();
        format!("({})", var_strs.join(", "))
    };

    lines.push(format!("{} {}{} {{", keyword, name, vars));

    // Selection set
    if let Some(sel) = selection {
        emit_selection_set(&mut lines, sel, 1);
    }

    lines.push("}".to_string());

    // Append any referenced fragment definitions
    let referenced = collect_fragment_refs(selection);
    for frag in fragments {
        if referenced.contains(&frag.name.as_str()) {
            lines.push(String::new());
            lines.push(format!("fragment {} on {} {{", frag.name, frag.type_condition));
            emit_selection_set(&mut lines, &frag.selection, 1);
            lines.push("}".to_string());
        }
    }

    lines.join("\n")
}

/// Emit a selection set at the given indentation level.
fn emit_selection_set(lines: &mut Vec<String>, sel: &SelectionSet, indent: usize) {
    let pad = "  ".repeat(indent);

    for field in &sel.fields {
        let alias = field
            .alias
            .as_ref()
            .map(|a| format!("{}: ", a))
            .unwrap_or_default();

        let args = if field.arguments.is_empty() {
            String::new()
        } else {
            let arg_strs: Vec<String> = field
                .arguments
                .iter()
                .map(|(name, _)| format!("{}: ${}", name, name))
                .collect();
            format!("({})", arg_strs.join(", "))
        };

        if let Some(nested) = &field.selection {
            lines.push(format!("{}{}{}{} {{", pad, alias, field.name, args));
            emit_selection_set(lines, nested, indent + 1);
            lines.push(format!("{}}}", pad));
        } else {
            lines.push(format!("{}{}{}{}", pad, alias, field.name, args));
        }
    }

    for spread in &sel.fragment_spreads {
        lines.push(format!("{}...{}", pad, spread.name));
    }

    for inline in &sel.inline_fragments {
        let condition = inline
            .type_condition
            .as_ref()
            .map(|tc| format!(" on {}", tc))
            .unwrap_or_default();
        lines.push(format!("{}...{} {{", pad, condition));
        emit_selection_set(lines, &inline.selection, indent + 1);
        lines.push(format!("{}}}", pad));
    }
}

/// Collect all fragment names referenced in a selection set tree.
fn collect_fragment_refs<'a>(selection: &'a Option<SelectionSet>) -> Vec<&'a str> {
    let mut refs = Vec::new();
    if let Some(sel) = selection {
        collect_fragment_refs_inner(sel, &mut refs);
    }
    refs
}

fn collect_fragment_refs_inner<'a>(sel: &'a SelectionSet, refs: &mut Vec<&'a str>) {
    for spread in &sel.fragment_spreads {
        if !refs.contains(&spread.name.as_str()) {
            refs.push(&spread.name);
        }
    }
    for field in &sel.fields {
        if let Some(nested) = &field.selection {
            collect_fragment_refs_inner(nested, refs);
        }
    }
    for inline in &sel.inline_fragments {
        collect_fragment_refs_inner(&inline.selection, refs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use odk_schema::{CompiledType, FieldSelection, FragmentDefinition, ScalarType, SelectionSet, Variable};

    #[test]
    fn test_scalar_to_ts() {
        assert_eq!(type_to_ts(&CompiledType::Scalar { name: ScalarType::String }), "string");
        assert_eq!(type_to_ts(&CompiledType::Scalar { name: ScalarType::Int32 }), "number");
        assert_eq!(type_to_ts(&CompiledType::Scalar { name: ScalarType::Boolean }), "boolean");
        assert_eq!(type_to_ts(&CompiledType::Scalar { name: ScalarType::Unknown }), "unknown");
    }

    #[test]
    fn test_array_to_ts() {
        let arr = CompiledType::Array {
            items: Box::new(CompiledType::Scalar { name: ScalarType::String }),
        };
        assert_eq!(type_to_ts(&arr), "string[]");
    }

    #[test]
    fn test_ref_to_ts() {
        assert_eq!(type_to_ts(&CompiledType::Ref { name: "User".into() }), "User");
    }

    #[test]
    fn test_naming_conventions() {
        assert_eq!(to_method_name("GetUser"), "getUser");
        assert_eq!(to_method_name("CreatePost"), "createPost");
        assert_eq!(to_hook_name("GetUser"), "useGetUser");
        assert_eq!(to_mock_name("GetUser"), "mockGetUser");
        assert_eq!(to_mock_data_name("GetUser"), "mockGetUserData");
        assert_eq!(to_mock_error_name("GetUser"), "mockGetUserError");
    }

    #[test]
    fn test_variable_to_graphql() {
        let var = Variable {
            name: "id".into(),
            variable_type: CompiledType::Scalar { name: ScalarType::String },
            required: true,
            default_value: None,
            description: None,
        };
        assert_eq!(variable_to_graphql(&var), "String!");

        let opt_var = Variable {
            name: "limit".into(),
            variable_type: CompiledType::Scalar { name: ScalarType::Int32 },
            required: false,
            default_value: None,
            description: None,
        };
        assert_eq!(variable_to_graphql(&opt_var), "Int");
    }

    #[test]
    fn test_reconstruct_simple_query() {
        let selection = Some(SelectionSet {
            fields: vec![
                FieldSelection {
                    name: "id".into(),
                    alias: None,
                    arguments: vec![],
                    selection: None,
                    directives: vec![],
                },
                FieldSelection {
                    name: "name".into(),
                    alias: None,
                    arguments: vec![],
                    selection: None,
                    directives: vec![],
                },
            ],
            fragment_spreads: vec![],
            inline_fragments: vec![],
        });

        let variables = vec![Variable {
            name: "id".into(),
            variable_type: CompiledType::Scalar { name: ScalarType::String },
            required: true,
            default_value: None,
            description: None,
        }];

        let result = reconstruct_operation(
            "GetUser",
            &OperationKind::Query,
            &variables,
            &selection,
            &[],
        );

        assert!(result.contains("query GetUser($id: String!)"));
        assert!(result.contains("  id"));
        assert!(result.contains("  name"));
    }

    #[test]
    fn test_reconstruct_with_nested_selection() {
        let selection = Some(SelectionSet {
            fields: vec![FieldSelection {
                name: "user".into(),
                alias: None,
                arguments: vec![],
                selection: Some(SelectionSet {
                    fields: vec![
                        FieldSelection {
                            name: "id".into(),
                            alias: None,
                            arguments: vec![],
                            selection: None,
                            directives: vec![],
                        },
                        FieldSelection {
                            name: "posts".into(),
                            alias: None,
                            arguments: vec![],
                            selection: Some(SelectionSet {
                                fields: vec![FieldSelection {
                                    name: "title".into(),
                                    alias: None,
                                    arguments: vec![],
                                    selection: None,
                                    directives: vec![],
                                }],
                                fragment_spreads: vec![],
                                inline_fragments: vec![],
                            }),
                            directives: vec![],
                        },
                    ],
                    fragment_spreads: vec![],
                    inline_fragments: vec![],
                }),
                directives: vec![],
            }],
            fragment_spreads: vec![],
            inline_fragments: vec![],
        });

        let result = reconstruct_operation(
            "GetUser",
            &OperationKind::Query,
            &[],
            &selection,
            &[],
        );

        assert!(result.contains("user {"));
        assert!(result.contains("    posts {"));
        assert!(result.contains("      title"));
    }

    #[test]
    fn test_reconstruct_with_fragment() {
        let selection = Some(SelectionSet {
            fields: vec![FieldSelection {
                name: "user".into(),
                alias: None,
                arguments: vec![],
                selection: Some(SelectionSet {
                    fields: vec![],
                    fragment_spreads: vec![odk_schema::FragmentSpread {
                        name: "UserFields".into(),
                        directives: vec![],
                    }],
                    inline_fragments: vec![],
                }),
                directives: vec![],
            }],
            fragment_spreads: vec![],
            inline_fragments: vec![],
        });

        let fragments = vec![FragmentDefinition {
            name: "UserFields".into(),
            type_condition: "User".into(),
            selection: SelectionSet {
                fields: vec![
                    FieldSelection {
                        name: "id".into(),
                        alias: None,
                        arguments: vec![],
                        selection: None,
                        directives: vec![],
                    },
                    FieldSelection {
                        name: "name".into(),
                        alias: None,
                        arguments: vec![],
                        selection: None,
                        directives: vec![],
                    },
                ],
                fragment_spreads: vec![],
                inline_fragments: vec![],
            },
            directives: vec![],
        }];

        let result = reconstruct_operation(
            "GetUser",
            &OperationKind::Query,
            &[],
            &selection,
            &fragments,
        );

        assert!(result.contains("...UserFields"));
        assert!(result.contains("fragment UserFields on User"));
    }

    #[test]
    fn test_target_parsing() {
        use super::super::Target;

        assert_eq!(Target::parse("sdk"), Some(Target::Sdk));
        assert_eq!(Target::parse("react-query"), Some(Target::ReactQuery));
        assert_eq!(Target::parse("msw"), Some(Target::Msw));
        assert_eq!(Target::parse("unknown"), None);

        let targets = Target::parse_list("sdk,react-query,msw").unwrap();
        assert_eq!(targets.len(), 3);
    }

    #[test]
    fn test_mcp_target_parsing() {
        use super::super::Target;

        assert_eq!(Target::parse("mcp"), Some(Target::Mcp));
        let targets = Target::parse_list("sdk,mcp").unwrap();
        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&Target::Mcp));
    }
}
