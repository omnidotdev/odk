//! MCP target — Model Context Protocol server.
//!
//! Generates a standalone MCP server that wraps GraphQL operations as
//! tools, with Zod input schemas and HTTP (Streamable HTTP) transport.

use crate::config::{McpGenerateConfig, McpToolParameter};
use odk_schema::{CompiledField, CompiledModel, CompiledSchema, CompiledType, ScalarType};
use std::collections::BTreeMap;

use super::util;

/// Generate the MCP server artifacts.
pub fn generate(schema: &CompiledSchema, config: &McpGenerateConfig) -> BTreeMap<String, String> {
    let mut output = BTreeMap::new();

    if config.tools.is_empty() {
        return output;
    }

    output.insert("server.ts".to_string(), generate_server(schema, config));
    output.insert("package.json".to_string(), generate_package_json(config));

    output
}

fn generate_server(schema: &CompiledSchema, config: &McpGenerateConfig) -> String {
    let mut lines = vec![
        "import { McpServer } from \"@modelcontextprotocol/sdk/server/mcp.js\";".to_string(),
        "import { WebStandardStreamableHTTPServerTransport } from \"@modelcontextprotocol/sdk/server/webStandardStreamableHttp.js\";".to_string(),
        "import { z } from \"zod\";".to_string(),
        "import { GraphQLClient } from \"graphql-request\";".to_string(),
        String::new(),
    ];

    // Emit set() helper if any tool uses custom parameters
    if needs_set_helper(config) {
        lines.extend(generate_set_helper());
        lines.push(String::new());
    }

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

    // GraphQL client
    lines.push("const graphql = new GraphQLClient(".to_string());
    lines.push("  process.env.GRAPHQL_ENDPOINT ?? \"http://localhost:4000/graphql\"".to_string());
    lines.push(");".to_string());
    lines.push(String::new());

    // Tool registration function
    let description = config
        .description
        .as_deref()
        .unwrap_or("Generated MCP server");

    let forward_auth = !config.forward_headers.is_empty();
    let headers_arg = if forward_auth { ", requestHeaders" } else { "" };
    let headers_arg_no_vars = if forward_auth {
        ", undefined, requestHeaders"
    } else {
        ""
    };

    if forward_auth {
        lines.push("function registerTools(server: McpServer, req: Request) {".to_string());
        let header_names: Vec<String> = config
            .forward_headers
            .iter()
            .map(|h| format!("\"{}\"", h.to_lowercase()))
            .collect();
        lines.push("  const requestHeaders: Record<string, string> = {};".to_string());
        lines.push(format!(
            "  for (const name of [{}]) {{",
            header_names.join(", ")
        ));
        lines.push("    const value = req.headers.get(name);".to_string());
        lines.push("    if (value !== null) requestHeaders[name] = value;".to_string());
        lines.push("  }".to_string());
        lines.push(String::new());
    } else {
        lines.push("function registerTools(server: McpServer) {".to_string());
    }

    for tool_cfg in &config.tools {
        let op = schema
            .operations
            .iter()
            .find(|o| o.name == tool_cfg.operation)
            .unwrap();

        let doc_name = format!("{}Document", op.name);

        lines.push("  server.tool(".to_string());
        lines.push(format!("    \"{}\",", tool_cfg.name));
        lines.push(format!("    \"{}\",", escape_str(&tool_cfg.description)));

        if tool_cfg.parameters.is_empty() {
            let zod_schema = build_zod_schema_for_variables(&op.variables, &schema.models);
            lines.push(format!("    {{{}}},", zod_schema));

            if op.variables.is_empty() {
                lines.push("    async () => {".to_string());
                lines.push(format!(
                    "      const data = await graphql.request({}{});",
                    doc_name, headers_arg_no_vars
                ));
            } else {
                let var_names: Vec<&str> = op.variables.iter().map(|v| v.name.as_str()).collect();
                let destructure = var_names.join(", ");
                lines.push(format!("    async ({{ {} }}) => {{", destructure));
                lines.push(format!(
                    "      const data = await graphql.request({}, {{ {} }}{});",
                    doc_name, destructure, headers_arg
                ));
            }
        } else {
            let zod_schema = build_zod_schema_for_params(&tool_cfg.parameters);
            lines.push(format!("    {{{}}},", zod_schema));
            lines.push("    async (params) => {".to_string());
            lines.push("      const variables: Record<string, unknown> = {};".to_string());
            for param in &tool_cfg.parameters {
                lines.push(format!(
                    "      if (params.{} !== undefined) set(variables, \"{}\", params.{});",
                    param.name, param.map_to, param.name
                ));
            }
            lines.push(format!(
                "      const data = await graphql.request({}, variables{});",
                doc_name, headers_arg
            ));
        }

        lines.push(
            "      return { content: [{ type: \"text\", text: JSON.stringify(data, null, 2) }] };"
                .to_string(),
        );
        lines.push("    }".to_string());
        lines.push("  );".to_string());
        lines.push(String::new());
    }

    lines.push("}".to_string());
    lines.push(String::new());

    // HTTP server
    lines.push("const app = Bun.serve({".to_string());
    lines.push(format!(
        "  port: Number(process.env.MCP_PORT ?? {}),",
        config.port
    ));
    lines.push("  async fetch(req) {".to_string());
    lines.push("    const url = new URL(req.url);".to_string());
    lines.push(format!("    if (url.pathname === \"{}\") {{", config.path));
    lines.push(
        "      const transport = new WebStandardStreamableHTTPServerTransport({ sessionIdGenerator: undefined, enableJsonResponse: true });"
            .to_string(),
    );
    lines.push(format!(
        "      const server = new McpServer({{ name: \"{}\", description: \"{}\", version: \"1.0.0\" }});",
        config.name, escape_str(description)
    ));
    if forward_auth {
        lines.push("      registerTools(server, req);".to_string());
    } else {
        lines.push("      registerTools(server);".to_string());
    }
    lines.push("      await server.connect(transport);".to_string());
    lines.push("      const response = await transport.handleRequest(req);".to_string());
    lines.push("      await transport.close();".to_string());
    lines.push("      await server.close();".to_string());
    lines.push("      return response;".to_string());
    lines.push("    }".to_string());
    lines.push("    return new Response(\"Not Found\", { status: 404 });".to_string());
    lines.push("  },".to_string());
    lines.push("});".to_string());
    lines.push(String::new());
    lines.push(format!(
        "console.error(`{} listening on port ${{app.port}}`);",
        config.name
    ));

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

/// Check if any tool uses custom parameters (needs the set() helper).
fn needs_set_helper(config: &McpGenerateConfig) -> bool {
    config.tools.iter().any(|t| !t.parameters.is_empty())
}

/// Generate the set() helper function for dot-path variable assignment.
fn generate_set_helper() -> Vec<String> {
    vec![
        "function set(obj: Record<string, unknown>, path: string, value: unknown) {".to_string(),
        "  const keys = path.split(\".\");".to_string(),
        "  let current = obj;".to_string(),
        "  for (let i = 0; i < keys.length - 1; i++) {".to_string(),
        "    if (!(keys[i] in current)) current[keys[i]] = {};".to_string(),
        "    current = current[keys[i]] as Record<string, unknown>;".to_string(),
        "  }".to_string(),
        "  current[keys[keys.length - 1]] = value;".to_string(),
        "}".to_string(),
    ]
}

/// Build Zod schema entries from curated parameter definitions.
fn build_zod_schema_for_params(params: &[McpToolParameter]) -> String {
    let entries: Vec<String> = params
        .iter()
        .map(|p| format!(" {}: {}", p.name, param_to_zod(p)))
        .collect();
    entries.join(",")
}

/// Convert a single McpToolParameter to a Zod expression.
fn param_to_zod(param: &McpToolParameter) -> String {
    let mut zod = if !param.enum_values.is_empty() {
        let quoted: Vec<String> = param
            .enum_values
            .iter()
            .map(|v| format!("\"{}\"", v))
            .collect();
        format!("z.enum([{}])", quoted.join(", "))
    } else {
        match param.type_.as_str() {
            "number" => "z.number()".to_string(),
            "boolean" => "z.boolean()".to_string(),
            _ => "z.string()".to_string(),
        }
    };

    // Apply format refinements (only for non-enum string types)
    if param.enum_values.is_empty()
        && param.type_ == "string"
        && let Some(fmt) = &param.format
    {
        match fmt.as_str() {
            "uuid" => zod = format!("{}.uuid()", zod),
            "datetime" => zod = format!("{}.datetime()", zod),
            "email" => zod = format!("{}.email()", zod),
            _ => {}
        }
    }

    // Apply default
    if let Some(default) = &param.default {
        zod = format!("{}.default({})", zod, default);
    }

    // Apply optionality
    if !param.required {
        zod = format!("{}.optional()", zod);
    }

    // Apply description
    if let Some(desc) = &param.description {
        zod = format!("{}.describe(\"{}\")", zod, escape_str(desc));
    }

    zod
}

fn generate_package_json(config: &McpGenerateConfig) -> String {
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
    use crate::config::{McpGenerateConfig, McpToolConfig};
    use odk_schema::*;

    fn test_config() -> McpGenerateConfig {
        McpGenerateConfig {
            name: "test-mcp".to_string(),
            description: Some("Test MCP server".to_string()),
            port: 4001,
            path: "/mcp".to_string(),
            tools: vec![
                McpToolConfig {
                    operation: "GetUser".to_string(),
                    name: "get_user".to_string(),
                    description: "Get a user by ID".to_string(),
                    parameters: vec![],
                },
                McpToolConfig {
                    operation: "CreatePost".to_string(),
                    name: "create_post".to_string(),
                    description: "Create a new post".to_string(),
                    parameters: vec![],
                },
            ],
            forward_headers: vec![],
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
        let config = McpGenerateConfig {
            name: "empty".to_string(),
            description: None,
            port: 4001,
            path: "/mcp".to_string(),
            tools: vec![],
            forward_headers: vec![],
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
        assert!(server.contains("@modelcontextprotocol/sdk/server/webStandardStreamableHttp.js"));
        assert!(server.contains("import { z } from \"zod\""));
        assert!(server.contains("import { GraphQLClient } from \"graphql-request\""));
        assert!(!server.contains("stdio"));
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
    fn test_server_has_http_transport() {
        let schema = test_schema();
        let config = test_config();
        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        assert!(server.contains("Bun.serve("));
        assert!(server.contains("WebStandardStreamableHTTPServerTransport"));
        assert!(server.contains("registerTools(server)"));
        assert!(server.contains("transport.handleRequest(req)"));
        assert!(server.contains("url.pathname === \"/mcp\""));
    }

    #[test]
    fn test_server_has_register_tools_function() {
        let schema = test_schema();
        let config = test_config();
        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        assert!(server.contains("function registerTools(server: McpServer)"));
    }

    #[test]
    fn test_default_emit_does_not_forward_headers() {
        let schema = test_schema();
        let config = test_config();
        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        assert!(!server.contains("requestHeaders"));
        assert!(!server.contains("registerTools(server, req)"));
        assert!(server.contains("registerTools(server)"));
    }

    #[test]
    fn test_forward_headers_threads_through_request_calls() {
        let schema = test_schema();
        let mut config = test_config();
        config.forward_headers = vec!["authorization".to_string()];
        let output = generate(&schema, &config);
        let server = &output["server.ts"];

        // registerTools accepts req and is called with it
        assert!(server.contains("function registerTools(server: McpServer, req: Request)"));
        assert!(server.contains("registerTools(server, req)"));

        // Builds requestHeaders from listed names
        assert!(server.contains("const requestHeaders: Record<string, string> = {};"));
        assert!(server.contains("for (const name of [\"authorization\"]) {"));
        assert!(server.contains("const value = req.headers.get(name);"));

        // Both auto-vars and (when present) curated tool calls thread requestHeaders through
        assert!(server.contains(", { id }, requestHeaders)"));
        assert!(server.contains(", { input }, requestHeaders)"));
    }

    #[test]
    fn test_forward_headers_lowercase_normalization() {
        let schema = test_schema();
        let mut config = test_config();
        config.forward_headers = vec!["Authorization".to_string(), "X-Trace-Id".to_string()];
        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        // Header names are normalized to lowercase for case-insensitive matching
        assert!(server.contains("[\"authorization\", \"x-trace-id\"]"));
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
        let config = McpGenerateConfig {
            name: "test".to_string(),
            description: None,
            port: 4001,
            path: "/mcp".to_string(),
            tools: vec![McpToolConfig {
                operation: "NonExistent".to_string(),
                name: "bad_tool".to_string(),
                description: "This should fail".to_string(),
                parameters: vec![],
            }],
            forward_headers: vec![],
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

    // --- Custom parameter tests ---

    fn test_param(name: &str, type_: &str, map_to: &str) -> McpToolParameter {
        McpToolParameter {
            name: name.to_string(),
            type_: type_.to_string(),
            format: None,
            enum_values: vec![],
            required: false,
            default: None,
            description: None,
            map_to: map_to.to_string(),
        }
    }

    #[test]
    fn test_custom_params_zod_schema() {
        let param = McpToolParameter {
            name: "projectId".to_string(),
            type_: "string".to_string(),
            format: Some("uuid".to_string()),
            enum_values: vec![],
            required: false,
            default: None,
            description: Some("Filter by project".to_string()),
            map_to: "condition.projectId".to_string(),
        };
        let zod = param_to_zod(&param);
        assert!(zod.contains("z.string()"));
        assert!(zod.contains(".uuid()"));
        assert!(zod.contains(".optional()"));
        assert!(zod.contains(".describe(\"Filter by project\")"));
    }

    #[test]
    fn test_custom_params_enum() {
        let param = McpToolParameter {
            name: "priority".to_string(),
            type_: "string".to_string(),
            format: None,
            enum_values: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
            required: false,
            default: None,
            description: None,
            map_to: "condition.priority".to_string(),
        };
        let zod = param_to_zod(&param);
        assert!(zod.contains("z.enum([\"low\", \"medium\", \"high\"])"));
        assert!(zod.contains(".optional()"));
        // enum should NOT have .uuid() or other format
        assert!(!zod.contains(".uuid()"));
    }

    #[test]
    fn test_custom_params_required() {
        let param = McpToolParameter {
            name: "content".to_string(),
            type_: "string".to_string(),
            format: None,
            enum_values: vec![],
            required: true,
            default: None,
            description: Some("Task title".to_string()),
            map_to: "input.task.content".to_string(),
        };
        let zod = param_to_zod(&param);
        assert!(zod.contains("z.string()"));
        assert!(!zod.contains(".optional()"));
        assert!(zod.contains(".describe(\"Task title\")"));
    }

    #[test]
    fn test_custom_params_default_value() {
        let param = McpToolParameter {
            name: "limit".to_string(),
            type_: "number".to_string(),
            format: None,
            enum_values: vec![],
            required: false,
            default: Some(serde_json::json!(20)),
            description: None,
            map_to: "first".to_string(),
        };
        let zod = param_to_zod(&param);
        assert!(zod.contains("z.number()"));
        assert!(zod.contains(".default(20)"));
        assert!(zod.contains(".optional()"));
    }

    #[test]
    fn test_custom_params_variable_builder() {
        let mut schema = CompiledSchema::new("Test");
        schema.add_operation(CompiledOperation {
            name: "tasks".into(),
            kind: OperationKind::Query,
            input: None,
            output: CompiledType::Scalar {
                name: ScalarType::Unknown,
            },
            variables: vec![],
            selection: None,
            description: None,
            extensions: Extensions::default(),
            directives: vec![],
        });

        let config = McpGenerateConfig {
            name: "test".to_string(),
            description: None,
            port: 4001,
            path: "/mcp".to_string(),
            tools: vec![McpToolConfig {
                operation: "tasks".to_string(),
                name: "list_tasks".to_string(),
                description: "List tasks".to_string(),
                parameters: vec![
                    test_param("projectId", "string", "condition.projectId"),
                    test_param("priority", "string", "condition.priority"),
                ],
            }],
            forward_headers: vec![],
        };

        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        assert!(server.contains("async (params)"));
        assert!(server.contains("const variables: Record<string, unknown> = {};"));
        assert!(server.contains("set(variables, \"condition.projectId\", params.projectId)"));
        assert!(server.contains("set(variables, \"condition.priority\", params.priority)"));
    }

    #[test]
    fn test_custom_params_set_helper_present() {
        let mut schema = CompiledSchema::new("Test");
        schema.add_operation(CompiledOperation {
            name: "tasks".into(),
            kind: OperationKind::Query,
            input: None,
            output: CompiledType::Scalar {
                name: ScalarType::Unknown,
            },
            variables: vec![],
            selection: None,
            description: None,
            extensions: Extensions::default(),
            directives: vec![],
        });

        let config = McpGenerateConfig {
            name: "test".to_string(),
            description: None,
            port: 4001,
            path: "/mcp".to_string(),
            tools: vec![McpToolConfig {
                operation: "tasks".to_string(),
                name: "list_tasks".to_string(),
                description: "List".to_string(),
                parameters: vec![test_param("x", "string", "a.b")],
            }],
            forward_headers: vec![],
        };

        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        assert!(server.contains("function set(obj: Record<string, unknown>"));
    }

    #[test]
    fn test_no_set_helper_without_params() {
        let schema = test_schema();
        let config = test_config(); // no parameters on any tool
        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        assert!(!server.contains("function set("));
    }

    #[test]
    fn test_mixed_tools() {
        let mut schema = CompiledSchema::new("Test");
        schema.add_operation(CompiledOperation {
            name: "GetUser".into(),
            kind: OperationKind::Query,
            input: None,
            output: CompiledType::Scalar {
                name: ScalarType::Unknown,
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
                    name: "id".into(),
                    alias: None,
                    arguments: vec![],
                    selection: None,
                    directives: vec![],
                }],
                fragment_spreads: vec![],
                inline_fragments: vec![],
            }),
            description: None,
            extensions: Extensions::default(),
            directives: vec![],
        });
        schema.add_operation(CompiledOperation {
            name: "tasks".into(),
            kind: OperationKind::Query,
            input: None,
            output: CompiledType::Scalar {
                name: ScalarType::Unknown,
            },
            variables: vec![],
            selection: None,
            description: None,
            extensions: Extensions::default(),
            directives: vec![],
        });

        let config = McpGenerateConfig {
            name: "mixed".to_string(),
            description: None,
            port: 4001,
            path: "/mcp".to_string(),
            tools: vec![
                // No parameters — uses existing path
                McpToolConfig {
                    operation: "GetUser".to_string(),
                    name: "get_user".to_string(),
                    description: "Get user".to_string(),
                    parameters: vec![],
                },
                // With parameters — uses custom path
                McpToolConfig {
                    operation: "tasks".to_string(),
                    name: "list_tasks".to_string(),
                    description: "List tasks".to_string(),
                    parameters: vec![test_param("projectId", "string", "condition.projectId")],
                },
            ],
            forward_headers: vec![],
        };

        let output = generate(&schema, &config);
        let server = &output["server.ts"];
        // set helper present (one tool uses params)
        assert!(server.contains("function set("));
        // GetUser uses old path
        assert!(server.contains("async ({ id })"));
        // tasks uses new path
        assert!(server.contains("async (params)"));
        assert!(server.contains("set(variables,"));
    }
}
