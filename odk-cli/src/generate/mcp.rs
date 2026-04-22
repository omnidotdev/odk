//! MCP target — Model Context Protocol server.
//!
//! Generates a standalone MCP server that wraps GraphQL operations as
//! tools, with Zod input schemas and STDIO transport.

use crate::config::McpConfig;
use odk_schema::{CompiledField, CompiledModel, CompiledSchema, CompiledType, ScalarType};
use std::collections::BTreeMap;

use super::util;

/// Generate the MCP server artifacts.
pub fn generate(schema: &CompiledSchema, config: &McpConfig) -> BTreeMap<String, String> {
    let mut output = BTreeMap::new();

    if config.tools.is_empty() {
        return output;
    }

    output.insert("server.ts".to_string(), generate_server(schema, config));
    output.insert("package.json".to_string(), generate_package_json(config));

    output
}

fn generate_server(schema: &CompiledSchema, config: &McpConfig) -> String {
    let mut lines = vec![
        "import { McpServer } from \"@modelcontextprotocol/sdk/server/mcp.js\";".to_string(),
        "import { StdioServerTransport } from \"@modelcontextprotocol/sdk/server/stdio.js\";"
            .to_string(),
        "import { z } from \"zod\";".to_string(),
        "import { GraphQLClient } from \"graphql-request\";".to_string(),
        String::new(),
    ];

    // GraphQL operation documents
    for tool_cfg in &config.tools {
        let op = schema
            .operations
            .iter()
            .find(|o| o.name == tool_cfg.operation)
            .unwrap_or_else(|| {
                panic!(
                    "MCP tool '{}' references operation '{}' which was not found in the schema",
                    tool_cfg.name, tool_cfg.operation
                )
            });

        let doc_name = format!("{}Document", op.name);
        let doc_text = util::reconstruct_operation(
            &op.name,
            &op.kind,
            &op.variables,
            &op.selection,
            &schema.fragments,
        );

        lines.push(format!("const {} = `", doc_name));
        for doc_line in doc_text.lines() {
            lines.push(format!("  {}", doc_line));
        }
        lines.push("`;".to_string());
        lines.push(String::new());
    }

    // Server setup
    let description = config
        .description
        .as_deref()
        .unwrap_or("Generated MCP server");

    lines.push("const server = new McpServer({".to_string());
    lines.push(format!("  name: \"{}\",", config.name));
    lines.push(format!("  description: \"{}\",", escape_str(description)));
    lines.push("  version: \"1.0.0\",".to_string());
    lines.push("});".to_string());
    lines.push(String::new());

    // GraphQL client
    lines.push("const graphql = new GraphQLClient(".to_string());
    lines.push("  process.env.GRAPHQL_ENDPOINT ?? \"http://localhost:4000/graphql\"".to_string());
    lines.push(");".to_string());
    lines.push(String::new());

    // Tool registrations
    for tool_cfg in &config.tools {
        let op = schema
            .operations
            .iter()
            .find(|o| o.name == tool_cfg.operation)
            .unwrap();

        let doc_name = format!("{}Document", op.name);
        let zod_schema = build_zod_schema_for_variables(&op.variables, &schema.models);

        lines.push("server.tool(".to_string());
        lines.push(format!("  \"{}\",", tool_cfg.name));
        lines.push(format!("  \"{}\",", escape_str(&tool_cfg.description)));
        lines.push(format!("  {{{}}},", zod_schema));

        if op.variables.is_empty() {
            lines.push("  async () => {".to_string());
            lines.push(format!(
                "    const data = await graphql.request({});",
                doc_name
            ));
        } else {
            let var_names: Vec<&str> = op.variables.iter().map(|v| v.name.as_str()).collect();
            let destructure = var_names.join(", ");
            lines.push(format!("  async ({{ {} }}) => {{", destructure));
            lines.push(format!(
                "    const data = await graphql.request({}, {{ {} }});",
                doc_name, destructure
            ));
        }

        lines.push(
            "    return { content: [{ type: \"text\", text: JSON.stringify(data, null, 2) }] };"
                .to_string(),
        );
        lines.push("  }".to_string());
        lines.push(");".to_string());
        lines.push(String::new());
    }

    // Start transport
    lines.push("const transport = new StdioServerTransport();".to_string());
    lines.push("await server.connect(transport);".to_string());

    lines.join("\n")
}

/// Build Zod schema entries for an operation's variables.
fn build_zod_schema_for_variables(
    variables: &[odk_schema::Variable],
    models: &[CompiledModel],
) -> String {
    if variables.is_empty() {
        return String::new();
    }

    let entries: Vec<String> = variables
        .iter()
        .map(|v| {
            let zod = compiled_type_to_zod(&v.variable_type, models);
            let zod = if !v.required {
                format!("{}.optional()", zod)
            } else {
                zod
            };
            format!(" {}: {}", v.name, zod)
        })
        .collect();

    entries.join(",")
}

/// Map a CompiledType to a Zod schema expression.
fn compiled_type_to_zod(t: &CompiledType, models: &[CompiledModel]) -> String {
    match t {
        CompiledType::Scalar { name } => scalar_to_zod(name).to_string(),
        CompiledType::Array { items } => {
            format!("z.array({})", compiled_type_to_zod(items, models))
        }
        CompiledType::Ref { name } => resolve_ref_to_zod(name, models),
        CompiledType::Object { fields } => object_fields_to_zod(fields, models),
        CompiledType::Union { variants } => {
            let parts: Vec<String> = variants
                .iter()
                .map(|v| compiled_type_to_zod(v, models))
                .collect();
            format!("z.union([{}])", parts.join(", "))
        }
        CompiledType::Literal { value } => match value {
            odk_schema::LiteralValue::String(s) => format!("z.literal(\"{}\")", s),
            odk_schema::LiteralValue::Number(n) => format!("z.literal({})", n),
            odk_schema::LiteralValue::Bool(b) => format!("z.literal({})", b),
        },
    }
}

fn scalar_to_zod(s: &ScalarType) -> &'static str {
    match s {
        ScalarType::String
        | ScalarType::Date
        | ScalarType::Datetime
        | ScalarType::UtcDateTime
        | ScalarType::OffsetDateTime
        | ScalarType::PlainDate
        | ScalarType::PlainTime
        | ScalarType::Duration
        | ScalarType::Bytes
        | ScalarType::Url => "z.string()",
        ScalarType::Int32 | ScalarType::Int64 | ScalarType::Float32 | ScalarType::Float64 => {
            "z.number()"
        }
        ScalarType::Boolean => "z.boolean()",
        ScalarType::Unknown => "z.unknown()",
    }
}

/// Resolve a Ref type by looking up the model and expanding its fields.
fn resolve_ref_to_zod(name: &str, models: &[CompiledModel]) -> String {
    if let Some(model) = models.iter().find(|m| m.name == name) {
        object_fields_to_zod(
            &model
                .fields
                .iter()
                .map(|f| CompiledField {
                    name: f.name.clone(),
                    field_type: f.field_type.clone(),
                    optional: f.optional,
                    description: f.description.clone(),
                    constraints: f.constraints.clone(),
                    extensions: f.extensions.clone(),
                    source: f.source.clone(),
                })
                .collect::<Vec<_>>(),
            models,
        )
    } else {
        format!("z.record(z.string(), z.unknown()).describe(\"{}\")", name)
    }
}

/// Convert inline object fields to a z.object() expression.
fn object_fields_to_zod(fields: &[CompiledField], models: &[CompiledModel]) -> String {
    let field_strs: Vec<String> = fields
        .iter()
        .map(|f| {
            let zod = compiled_type_to_zod(&f.field_type, models);
            let zod = if f.optional == Some(true) {
                format!("{}.optional()", zod)
            } else {
                zod
            };
            format!("{}: {}", f.name, zod)
        })
        .collect();
    format!("z.object({{ {} }})", field_strs.join(", "))
}

fn generate_package_json(config: &McpConfig) -> String {
    let description = config
        .description
        .as_deref()
        .unwrap_or("Generated MCP server");

    format!(
        r#"{{
  "name": "{}",
  "description": "{}",
  "type": "module",
  "private": true,
  "scripts": {{
    "start": "bun run server.ts"
  }},
  "dependencies": {{
    "@modelcontextprotocol/sdk": "^1.25.0",
    "graphql": "^16.0.0",
    "graphql-request": "^7.0.0",
    "zod": "^3.23.0"
  }}
}}"#,
        config.name,
        escape_str(description)
    )
}

/// Escape double quotes and backslashes in a string for embedding in generated code.
fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpConfig, McpToolConfig};
    use odk_schema::*;

    fn test_config() -> McpConfig {
        McpConfig {
            name: "test-mcp".to_string(),
            description: Some("Test MCP server".to_string()),
            tools: vec![
                McpToolConfig {
                    operation: "GetUser".to_string(),
                    name: "get_user".to_string(),
                    description: "Get a user by ID".to_string(),
                },
                McpToolConfig {
                    operation: "CreatePost".to_string(),
                    name: "create_post".to_string(),
                    description: "Create a new post".to_string(),
                },
            ],
        }
    }

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
            }),
            description: None,
            extensions: Extensions::default(),
            directives: vec![],
        });

        schema.add_operation(CompiledOperation {
            name: "CreatePost".into(),
            kind: OperationKind::Mutation,
            input: None,
            output: CompiledType::Ref {
                name: "Post".into(),
            },
            variables: vec![Variable {
                name: "input".into(),
                variable_type: CompiledType::Ref {
                    name: "CreatePostInput".into(),
                },
                required: true,
                default_value: None,
                description: None,
            }],
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
                        name: "title".into(),
                        alias: None,
                        arguments: vec![],
                        selection: None,
                        directives: vec![],
                    },
                ],
                fragment_spreads: vec![],
                inline_fragments: vec![],
            }),
            description: None,
            extensions: Extensions::default(),
            directives: vec![],
        });

        // Unrelated operation not in config — should be excluded
        schema.add_operation(CompiledOperation {
            name: "DeleteUser".into(),
            kind: OperationKind::Mutation,
            input: None,
            output: CompiledType::Scalar {
                name: ScalarType::Boolean,
            },
            variables: vec![],
            selection: None,
            description: None,
            extensions: Extensions::default(),
            directives: vec![],
        });

        schema
    }

    #[test]
    fn test_generates_server_and_package_json() {
        let schema = test_schema();
        let config = test_config();
        let output = generate(&schema, &config);
        assert!(output.contains_key("server.ts"));
        assert!(output.contains_key("package.json"));
    }

    #[test]
    fn test_empty_tools_no_output() {
        let schema = test_schema();
        let config = McpConfig {
            name: "empty".to_string(),
            description: None,
            tools: vec![],
        };
        let output = generate(&schema, &config);
        assert!(output.is_empty());
    }

    #[test]
    fn test_server_has_imports() {
        let schema = test_schema();
        let config = test_config();
        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        assert!(server.contains("@modelcontextprotocol/sdk/server/mcp.js"));
        assert!(server.contains("@modelcontextprotocol/sdk/server/stdio.js"));
        assert!(server.contains("import { z } from \"zod\""));
        assert!(server.contains("import { GraphQLClient } from \"graphql-request\""));
    }

    #[test]
    fn test_server_has_operation_documents() {
        let schema = test_schema();
        let config = test_config();
        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        assert!(server.contains("const GetUserDocument = `"));
        assert!(server.contains("const CreatePostDocument = `"));
        assert!(server.contains("query GetUser($id: String!)"));
        assert!(server.contains("mutation CreatePost($input: CreatePostInput!)"));
    }

    #[test]
    fn test_server_has_tool_registrations() {
        let schema = test_schema();
        let config = test_config();
        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        assert!(server.contains("\"get_user\""));
        assert!(server.contains("\"Get a user by ID\""));
        assert!(server.contains("\"create_post\""));
        assert!(server.contains("\"Create a new post\""));
    }

    #[test]
    fn test_operation_filtering() {
        let schema = test_schema();
        let config = test_config();
        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        // DeleteUser is in the schema but not in config.tools
        assert!(!server.contains("DeleteUser"));
        assert!(!server.contains("delete_user"));
    }

    #[test]
    fn test_server_has_transport_startup() {
        let schema = test_schema();
        let config = test_config();
        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        assert!(server.contains("new StdioServerTransport()"));
        assert!(server.contains("await server.connect(transport)"));
    }

    #[test]
    fn test_zod_scalar_mapping() {
        let models = vec![];
        assert_eq!(
            compiled_type_to_zod(
                &CompiledType::Scalar {
                    name: ScalarType::String
                },
                &models
            ),
            "z.string()"
        );
        assert_eq!(
            compiled_type_to_zod(
                &CompiledType::Scalar {
                    name: ScalarType::Int32
                },
                &models
            ),
            "z.number()"
        );
        assert_eq!(
            compiled_type_to_zod(
                &CompiledType::Scalar {
                    name: ScalarType::Boolean
                },
                &models
            ),
            "z.boolean()"
        );
        assert_eq!(
            compiled_type_to_zod(
                &CompiledType::Scalar {
                    name: ScalarType::Url
                },
                &models
            ),
            "z.string()"
        );
    }

    #[test]
    fn test_zod_array_mapping() {
        let models = vec![];
        let arr = CompiledType::Array {
            items: Box::new(CompiledType::Scalar {
                name: ScalarType::String,
            }),
        };
        assert_eq!(compiled_type_to_zod(&arr, &models), "z.array(z.string())");
    }

    #[test]
    fn test_zod_ref_with_model() {
        let models = vec![CompiledModel {
            name: "CreatePostInput".into(),
            fields: vec![
                CompiledField {
                    name: "title".into(),
                    field_type: CompiledType::Scalar {
                        name: ScalarType::String,
                    },
                    optional: None,
                    description: None,
                    constraints: None,
                    extensions: Extensions::default(),
                    source: None,
                },
                CompiledField {
                    name: "draft".into(),
                    field_type: CompiledType::Scalar {
                        name: ScalarType::Boolean,
                    },
                    optional: Some(true),
                    description: None,
                    constraints: None,
                    extensions: Extensions::default(),
                    source: None,
                },
            ],
            description: None,
            extensions: Extensions::default(),
            source: None,
        }];

        let result = compiled_type_to_zod(
            &CompiledType::Ref {
                name: "CreatePostInput".into(),
            },
            &models,
        );
        assert!(result.contains("z.object("));
        assert!(result.contains("title: z.string()"));
        assert!(result.contains("draft: z.boolean().optional()"));
    }

    #[test]
    fn test_zod_ref_without_model() {
        let models = vec![];
        let result = compiled_type_to_zod(
            &CompiledType::Ref {
                name: "UnknownType".into(),
            },
            &models,
        );
        assert!(result.contains("z.record("));
        assert!(result.contains("UnknownType"));
    }

    #[test]
    fn test_package_json_content() {
        let config = test_config();
        let pkg = generate_package_json(&config);
        assert!(pkg.contains("\"name\": \"test-mcp\""));
        assert!(pkg.contains("@modelcontextprotocol/sdk"));
        assert!(pkg.contains("graphql-request"));
        assert!(pkg.contains("zod"));
        assert!(pkg.contains("\"type\": \"module\""));
        assert!(pkg.contains("bun run server.ts"));
    }

    #[test]
    #[should_panic(expected = "references operation 'NonExistent' which was not found")]
    fn test_missing_operation_panics() {
        let schema = test_schema();
        let config = McpConfig {
            name: "test".to_string(),
            description: None,
            tools: vec![McpToolConfig {
                operation: "NonExistent".to_string(),
                name: "bad_tool".to_string(),
                description: "This should fail".to_string(),
            }],
        };
        generate(&schema, &config);
    }

    #[test]
    fn test_server_variable_destructuring() {
        let schema = test_schema();
        let config = test_config();
        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        // GetUser has a variable `id` — should destructure it
        assert!(server.contains("async ({ id })"));
        // CreatePost has a variable `input` — should destructure it
        assert!(server.contains("async ({ input })"));
    }
}
