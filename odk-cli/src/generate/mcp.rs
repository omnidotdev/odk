//! MCP target -- Model Context Protocol server.
//!
//! Generates a standalone MCP server package that exposes
//! curated GraphQL operations as MCP tools.

use crate::config::{McpGenerateConfig, McpToolConfig};
use odk_schema::{CompiledSchema, CompiledType, ScalarType, Variable};
use std::collections::BTreeMap;

use super::util;

/// Generate MCP server files (default mode, exposes all operations).
pub fn generate(schema: &CompiledSchema) -> BTreeMap<String, String> {
    // Build a default config that includes every operation
    let tools: Vec<McpToolConfig> = schema
        .operations
        .iter()
        .map(|op| McpToolConfig {
            operation: op.name.clone(),
            name: None,
            description: op.description.clone(),
        })
        .collect();

    let config = McpGenerateConfig {
        name: schema.name.clone(),
        description: String::new(),
        tools,
    };

    generate_with_config(schema, &config)
}

/// Generate MCP server files using a curated config.
///
/// Only operations listed in `config.tools` are exposed as MCP tools.
pub fn generate_with_config(
    schema: &CompiledSchema,
    config: &McpGenerateConfig,
) -> BTreeMap<String, String> {
    let mut output = BTreeMap::new();

    if config.tools.is_empty() {
        return output;
    }

    output.insert("index.ts".to_string(), generate_index(schema, config));
    output.insert("package.json".to_string(), generate_package_json(config));

    output
}

/// Generate the MCP server entrypoint (index.ts).
fn generate_index(schema: &CompiledSchema, config: &McpGenerateConfig) -> String {
    let mut lines = Vec::new();

    // Imports
    lines
        .push("import { McpServer } from \"@modelcontextprotocol/sdk/server/mcp.js\";".to_string());
    lines.push(
        "import { StdioServerTransport } from \"@modelcontextprotocol/sdk/server/stdio.js\";"
            .to_string(),
    );
    lines.push("import { GraphQLClient } from \"graphql-request\";".to_string());
    lines.push("import { z } from \"zod\";".to_string());
    lines.push(String::new());

    // GraphQL client setup
    lines.push("const API_URL = process.env.API_URL!;".to_string());
    lines.push("const OMNI_USER_TOKEN = process.env.OMNI_USER_TOKEN;".to_string());
    lines.push(String::new());
    lines.push("const gql = new GraphQLClient(API_URL, {".to_string());
    lines.push(
        "  headers: OMNI_USER_TOKEN ? { authorization: `Bearer ${OMNI_USER_TOKEN}` } : {},"
            .to_string(),
    );
    lines.push("});".to_string());
    lines.push(String::new());

    // Server instance
    lines.push(format!(
        "const server = new McpServer({{ name: \"{}\", version: \"1.0.0\" }});",
        escape_str(&config.name)
    ));
    lines.push(String::new());

    // Per-tool registration
    for tool_config in &config.tools {
        let op = match schema
            .operations
            .iter()
            .find(|o| o.name == tool_config.operation)
        {
            Some(op) => op,
            None => continue,
        };

        // Tool name: use override or snake_case of operation name
        let tool_name = tool_config
            .name
            .clone()
            .unwrap_or_else(|| util::to_method_name(&op.name));

        // Tool description
        let description = tool_config
            .description
            .clone()
            .or_else(|| op.description.clone())
            .unwrap_or_else(|| format!("{} operation", op.name));

        // Embed GraphQL document
        let doc_text = util::reconstruct_operation(
            &op.name,
            &op.kind,
            &op.variables,
            &op.selection,
            &schema.fragments,
        );

        let query_var_name = format!("{}Query", util::to_method_name(&op.name));
        lines.push(format!("const {} = `", query_var_name));
        for doc_line in doc_text.lines() {
            lines.push(format!("  {}", doc_line));
        }
        lines.push("`;".to_string());
        lines.push(String::new());

        // Build Zod input schema
        let zod_params = build_zod_params(&op.variables);

        // server.tool() registration
        lines.push(format!("server.tool(",));
        lines.push(format!("  \"{}\",", escape_str(&tool_name)));
        lines.push(format!("  \"{}\",", escape_str(&description)));

        if op.variables.is_empty() {
            lines.push("  {},".to_string());
        } else {
            lines.push(format!("  {{ {} }},", zod_params));
        }

        lines.push("  async (params) => {".to_string());

        if op.variables.is_empty() {
            lines.push(format!(
                "    const data = await gql.request({});",
                query_var_name
            ));
        } else {
            lines.push(format!(
                "    const data = await gql.request({}, params);",
                query_var_name
            ));
        }

        lines.push(
            "    return { content: [{ type: \"text\", text: JSON.stringify(data, null, 2) }] };"
                .to_string(),
        );
        lines.push("  },".to_string());
        lines.push(");".to_string());
        lines.push(String::new());
    }

    // Transport connection
    lines.push("const transport = new StdioServerTransport();".to_string());
    lines.push("server.connect(transport);".to_string());

    lines.join("\n")
}

/// Build Zod parameter declarations for operation variables.
fn build_zod_params(variables: &[Variable]) -> String {
    let parts: Vec<String> = variables
        .iter()
        .map(|v| {
            let zod = variable_to_zod(v);
            format!("{}: {}", v.name, zod)
        })
        .collect();

    parts.join(", ")
}

/// Convert a Variable to a Zod schema string.
fn variable_to_zod(var: &Variable) -> String {
    let base = compiled_type_to_zod(&var.variable_type);
    if !var.required {
        format!("{}.optional()", base)
    } else {
        base
    }
}

/// Convert a CompiledType to a Zod schema string.
fn compiled_type_to_zod(t: &CompiledType) -> String {
    match t {
        CompiledType::Scalar { name } => scalar_to_zod(name).to_string(),
        CompiledType::Array { items, .. } => {
            format!("z.array({})", compiled_type_to_zod(items))
        }
        CompiledType::Ref { name } => match name.as_str() {
            // Well-known custom scalars are string-encoded over the wire, so a
            // tool arg should be a plain string, not an opaque object. Only the
            // zod input schema is affected; the embedded GraphQL operation
            // document still uses the original scalar type name (e.g. UUID).
            "UUID" | "Cursor" | "Datetime" | "DateTime" => "z.string()".to_string(),
            // Input object types (and anything else) stay passthrough objects.
            _ => format!(
                "z.object({{}}).passthrough().describe(\"{}\")",
                escape_str(name)
            ),
        },
        CompiledType::Union { variants } => {
            let parts: Vec<String> = variants.iter().map(compiled_type_to_zod).collect();
            format!("z.union([{}])", parts.join(", "))
        }
        CompiledType::Literal { value } => match value {
            odk_schema::LiteralValue::String(s) => format!("z.literal(\"{}\")", escape_str(s)),
            odk_schema::LiteralValue::Number(n) => format!("z.literal({})", n),
            odk_schema::LiteralValue::Bool(b) => format!("z.literal({})", b),
        },
        CompiledType::Object { fields } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|f| {
                    let zod = compiled_type_to_zod(&f.field_type);
                    let zod = if f.optional == Some(true) {
                        format!("{}.optional()", zod)
                    } else {
                        zod
                    };
                    format!("{}: {}", f.name, zod)
                })
                .collect();
            format!("z.object({{ {} }})", parts.join(", "))
        }
    }
}

/// Map a ScalarType to its Zod schema string.
fn scalar_to_zod(s: &ScalarType) -> &'static str {
    match s {
        ScalarType::String => "z.string()",
        ScalarType::Int32 | ScalarType::Int64 => "z.number().int()",
        ScalarType::Float32 | ScalarType::Float64 => "z.number()",
        ScalarType::Boolean => "z.boolean()",
        ScalarType::Date
        | ScalarType::Datetime
        | ScalarType::UtcDateTime
        | ScalarType::OffsetDateTime
        | ScalarType::PlainDate
        | ScalarType::PlainTime
        | ScalarType::Duration
        | ScalarType::Bytes => "z.string()",
        ScalarType::Url => "z.string().url()",
        ScalarType::Unknown => "z.unknown()",
    }
}

/// Escape a string for safe embedding in a JS string literal.
fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Generate package.json for the MCP server.
fn generate_package_json(config: &McpGenerateConfig) -> String {
    let description = if config.description.is_empty() {
        format!("{} MCP server", config.name)
    } else {
        config.description.clone()
    };

    format!(
        r#"{{
  "name": "@omnidotdev/{}",
  "version": "0.1.0",
  "private": true,
  "description": "{}",
  "main": "index.ts",
  "scripts": {{
    "start": "bun run index.ts"
  }},
  "dependencies": {{
    "@modelcontextprotocol/sdk": "^1",
    "graphql": "^16",
    "graphql-request": "^7",
    "zod": "^3"
  }}
}}"#,
        escape_str(&config.name),
        escape_str(&description),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpGenerateConfig, McpToolConfig};
    use odk_schema::*;

    fn test_schema() -> CompiledSchema {
        let mut schema = CompiledSchema::new("TestAPI");

        schema.add_operation(CompiledOperation {
            name: "createPost".into(),
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

        schema.add_operation(CompiledOperation {
            name: "posts".into(),
            kind: OperationKind::Query,
            input: None,
            output: CompiledType::Array {
                items: Box::new(CompiledType::Ref {
                    name: "Post".into(),
                }),
                length: None,
            },
            variables: vec![
                Variable {
                    name: "first".into(),
                    variable_type: CompiledType::Scalar {
                        name: ScalarType::Int32,
                    },
                    required: false,
                    default_value: None,
                    description: None,
                },
                Variable {
                    name: "condition".into(),
                    variable_type: CompiledType::Ref {
                        name: "PostCondition".into(),
                    },
                    required: false,
                    default_value: None,
                    description: None,
                },
            ],
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

        schema
    }

    fn test_mcp_config() -> McpGenerateConfig {
        McpGenerateConfig {
            name: "test-mcp".into(),
            description: "Test MCP server".into(),
            tools: vec![
                McpToolConfig {
                    operation: "createPost".into(),
                    name: Some("submit_feedback".into()),
                    description: Some("Submit feedback to a board".into()),
                },
                McpToolConfig {
                    operation: "posts".into(),
                    name: None,
                    description: None,
                },
            ],
        }
    }

    #[test]
    fn test_generates_index_ts() {
        let schema = test_schema();
        let config = test_mcp_config();
        let output = generate_with_config(&schema, &config);
        assert!(output.contains_key("index.ts"));
    }

    #[test]
    fn test_index_has_mcp_imports() {
        let schema = test_schema();
        let config = test_mcp_config();
        let output = generate_with_config(&schema, &config);
        let index = &output["index.ts"];
        assert!(index.contains("McpServer"));
        assert!(index.contains("StdioServerTransport"));
    }

    #[test]
    fn test_index_has_tool_registrations() {
        let schema = test_schema();
        let config = test_mcp_config();
        let output = generate_with_config(&schema, &config);
        let index = &output["index.ts"];
        assert!(
            index.contains("submit_feedback"),
            "should contain renamed tool name"
        );
        assert!(index.contains("posts"), "should contain default tool name");
    }

    #[test]
    fn test_index_has_tool_descriptions() {
        let schema = test_schema();
        let config = test_mcp_config();
        let output = generate_with_config(&schema, &config);
        let index = &output["index.ts"];
        assert!(index.contains("Submit feedback to a board"));
    }

    #[test]
    fn test_generates_package_json() {
        let schema = test_schema();
        let config = test_mcp_config();
        let output = generate_with_config(&schema, &config);
        let pkg = &output["package.json"];
        assert!(pkg.contains("@omnidotdev/test-mcp"));
        assert!(pkg.contains("@modelcontextprotocol/sdk"));
        assert!(pkg.contains("graphql-request"));
    }

    #[test]
    fn test_index_has_input_schema() {
        let schema = test_schema();
        let config = test_mcp_config();
        let output = generate_with_config(&schema, &config);
        let index = &output["index.ts"];
        // createPost has an input variable of Ref type -> should produce z.object passthrough
        assert!(
            index.contains("z.object({}).passthrough()") || index.contains("z.number().int()"),
            "should contain Zod schema for variables"
        );
    }

    #[test]
    fn test_empty_config_tools_generates_nothing() {
        let schema = test_schema();
        let config = McpGenerateConfig {
            name: "empty".into(),
            description: String::new(),
            tools: vec![],
        };
        let output = generate_with_config(&schema, &config);
        assert!(output.is_empty());
    }
}
