//! MSW target — mock handlers and data factories.
//!
//! Generates `mocks.ts` containing:
//! - Per-operation MSW handler factories
//! - Mock data factories with sensible defaults
//! - Error mock helpers
//! - `allHandlers` convenience export

use odk_schema::{CompiledSchema, OperationKind};
use std::collections::BTreeMap;

use super::util;

/// Generate MSW mock handlers.
pub fn generate(schema: &CompiledSchema) -> BTreeMap<String, String> {
    let mut output = BTreeMap::new();

    if schema.operations.is_empty() {
        return output;
    }

    output.insert("mocks.ts".to_string(), generate_mocks(schema));
    output
}

fn generate_mocks(schema: &CompiledSchema) -> String {
    let mut lines = Vec::new();

    // Imports
    lines.push("import { graphql, HttpResponse } from \"msw\";".to_string());

    // Import result types from client
    let type_imports: Vec<String> = schema
        .operations
        .iter()
        .map(|op| format!("type {}Result", op.name))
        .collect();
    lines.push(format!(
        "import {{ {} }} from \"./client\";",
        type_imports.join(", ")
    ));
    lines.push(String::new());

    // Handler factories, mock data, and error helpers per operation
    for op in &schema.operations {
        let mock_name = util::to_mock_name(&op.name);
        let mock_data_name = util::to_mock_data_name(&op.name);
        let mock_error_name = util::to_mock_error_name(&op.name);

        let gql_method = match op.kind {
            OperationKind::Query => "query",
            OperationKind::Mutation => "mutation",
            _ => "query",
        };

        // Determine the root field name (first field in selection set)
        let root_field = op
            .selection
            .as_ref()
            .and_then(|s| s.fields.first())
            .map(|f| f.name.as_str())
            .unwrap_or("data");

        // Handler factory
        lines.push(format!(
            "export const {} = (resolver?: () => {}Result[\"{}\"],) =>",
            mock_name, op.name, root_field
        ));
        lines.push(format!("  graphql.{}(\"{}\", () =>", gql_method, op.name));
        lines.push("    HttpResponse.json({".to_string());
        lines.push(format!(
            "      data: {{ {}: resolver?.() ?? {}() }},",
            root_field, mock_data_name
        ));
        lines.push("    }),".to_string());
        lines.push("  );".to_string());
        lines.push(String::new());

        // Mock data factory
        lines.push(format!("export const {} = (", mock_data_name));
        lines.push(format!(
            "  overrides?: Partial<{}Result[\"{}\"]>,",
            op.name, root_field
        ));
        lines.push(format!("): {}Result[\"{}\"] => ({{", op.name, root_field));

        // Generate default values from the selection set
        if let Some(sel) = &op.selection {
            if let Some(root) = sel.fields.first() {
                if let Some(nested) = &root.selection {
                    for field in &nested.fields {
                        let default = guess_mock_default(&field.name, field.selection.is_some());
                        lines.push(format!("  {}: {},", field.name, default));
                    }
                }
            }
        }

        lines.push("  ...overrides,".to_string());
        lines.push("});".to_string());
        lines.push(String::new());

        // Error helper
        lines.push(format!(
            "export const {} = (message = \"An error occurred\") =>",
            mock_error_name
        ));
        lines.push(format!("  graphql.{}(\"{}\", () =>", gql_method, op.name));
        lines.push("    HttpResponse.json({".to_string());
        lines.push("      errors: [{ message }],".to_string());
        lines.push("    }),".to_string());
        lines.push("  );".to_string());
        lines.push(String::new());
    }

    // allHandlers convenience export
    let handler_calls: Vec<String> = schema
        .operations
        .iter()
        .map(|op| format!("{}()", util::to_mock_name(&op.name)))
        .collect();
    lines.push(format!(
        "export const allHandlers = [{}];",
        handler_calls.join(", ")
    ));

    lines.join("\n")
}

/// Guess a sensible mock default value for a field based on its name.
fn guess_mock_default(name: &str, has_children: bool) -> &'static str {
    if has_children {
        return "[]";
    }
    match name {
        "id" => "\"mock-id\"",
        "name" | "title" | "label" => "\"Mock Value\"",
        "email" => "\"mock@example.com\"",
        "url" | "href" | "link" => "\"https://example.com\"",
        "description" | "bio" | "body" | "content" => "\"Mock text\"",
        "createdAt" | "updatedAt" | "publishedAt" => "\"2024-01-01T00:00:00Z\"",
        "count" | "total" | "amount" | "price" | "quantity" | "size" | "length" | "age"
        | "width" | "height" | "weight" | "offset" | "limit" | "page" | "index" => "0",
        _ if name.ends_with("Count") || name.ends_with("Total") || name.ends_with("Amount") => "0",
        _ if name.ends_with("At") => "\"2024-01-01T00:00:00Z\"",
        _ if name.ends_with("Id") => "\"mock-id\"",
        _ if name.ends_with("Url") || name.ends_with("URI") => "\"https://example.com\"",
        _ if name.ends_with("Name") => "\"Mock Value\"",
        _ if name.ends_with("Email") => "\"mock@example.com\"",
        "active" | "enabled" | "published" | "verified" => "false",
        "slug" => "\"mock-slug\"",
        "avatar" | "image" | "thumbnail" => "\"https://example.com/image.png\"",
        _ => "\"\"",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use odk_schema::*;

    fn test_schema() -> CompiledSchema {
        let mut schema = CompiledSchema::new("TestAPI");

        schema.add_operation(CompiledOperation {
            name: "GetUser".into(),
            kind: OperationKind::Query,
            input: None,
            output: CompiledType::Ref {
                name: "User".into(),
            },
            variables: vec![Variable {
                name: "id".into(),
                variable_type: CompiledType::Scalar {
                    name: ScalarType::String,
                },
                required: true,
                default_value: None,
                description: None,
            }],
            selection: Some(SelectionSet {
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
                                name: "name".into(),
                                alias: None,
                                arguments: vec![],
                                selection: None,
                                directives: vec![],
                            },
                            FieldSelection {
                                name: "email".into(),
                                alias: None,
                                arguments: vec![],
                                selection: None,
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
            }),
            description: None,
            extensions: Extensions::default(),
            directives: vec![],
        });

        schema
    }

    #[test]
    fn test_generates_mocks_file() {
        let output = generate(&test_schema());
        assert!(output.contains_key("mocks.ts"));
    }

    #[test]
    fn test_mocks_has_handler() {
        let output = generate(&test_schema());
        let mocks = &output["mocks.ts"];
        assert!(mocks.contains("export const mockGetUser"));
        assert!(mocks.contains("graphql.query(\"GetUser\""));
    }

    #[test]
    fn test_mocks_has_data_factory() {
        let output = generate(&test_schema());
        let mocks = &output["mocks.ts"];
        assert!(mocks.contains("export const mockGetUserData"));
        assert!(mocks.contains("id: \"mock-id\""));
        assert!(mocks.contains("name: \"Mock Value\""));
        assert!(mocks.contains("email: \"mock@example.com\""));
        assert!(mocks.contains("...overrides"));
    }

    #[test]
    fn test_mocks_has_error_helper() {
        let output = generate(&test_schema());
        let mocks = &output["mocks.ts"];
        assert!(mocks.contains("export const mockGetUserError"));
        assert!(mocks.contains("errors: [{ message }]"));
    }

    #[test]
    fn test_mocks_has_all_handlers() {
        let output = generate(&test_schema());
        let mocks = &output["mocks.ts"];
        assert!(mocks.contains("export const allHandlers"));
        assert!(mocks.contains("mockGetUser()"));
    }
}
