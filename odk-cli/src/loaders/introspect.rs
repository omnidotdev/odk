//! GraphQL introspection client
//!
//! Fetch a schema from a running GraphQL endpoint via the standard
//! introspection query, convert the JSON response to SDL, then parse
//! it through the SDL parser.

use odk_schema::CompiledSchema;

use super::graphql::{self, GraphqlLoadError};

/// Standard introspection query that fetches the full schema.
const INTROSPECTION_QUERY: &str = r#"
query IntrospectionQuery {
  __schema {
    queryType { name }
    mutationType { name }
    subscriptionType { name }
    types {
      kind
      name
      description
      fields(includeDeprecated: true) {
        name
        description
        args {
          name
          description
          type { ...TypeRef }
          defaultValue
        }
        type { ...TypeRef }
        isDeprecated
        deprecationReason
      }
      inputFields {
        name
        description
        type { ...TypeRef }
        defaultValue
      }
      interfaces { ...TypeRef }
      enumValues(includeDeprecated: true) {
        name
        description
        isDeprecated
        deprecationReason
      }
      possibleTypes { ...TypeRef }
    }
    directives {
      name
      description
      locations
      args {
        name
        description
        type { ...TypeRef }
        defaultValue
      }
    }
  }
}

fragment TypeRef on __Type {
  kind
  name
  ofType {
    kind
    name
    ofType {
      kind
      name
      ofType {
        kind
        name
        ofType {
          kind
          name
          ofType {
            kind
            name
            ofType {
              kind
              name
            }
          }
        }
      }
    }
  }
}
"#;

/// Fetch a schema from a GraphQL endpoint via introspection.
pub fn introspect_schema(url: &str) -> Result<CompiledSchema, GraphqlLoadError> {
    let body = serde_json::json!({
        "query": INTROSPECTION_QUERY,
    });

    let response: serde_json::Value = ureq::post(url)
        .header("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|e| GraphqlLoadError::Validation(format!("HTTP request failed: {}", e)))?
        .body_mut()
        .read_json()
        .map_err(|e| {
            GraphqlLoadError::Validation(format!(
                "Failed to parse introspection response: {}",
                e
            ))
        })?;

    // Extract the schema from the response
    let schema_data = response
        .get("data")
        .and_then(|d| d.get("__schema"))
        .ok_or_else(|| {
            GraphqlLoadError::Validation(
                "Introspection response missing data.__schema".to_string(),
            )
        })?;

    // Convert introspection JSON to SDL, then parse through the SDL parser
    let sdl = introspection_to_sdl(schema_data)?;
    graphql::parse_graphql_sdl(&sdl, url)
}

/// Convert introspection JSON result to GraphQL SDL string.
fn introspection_to_sdl(schema: &serde_json::Value) -> Result<String, GraphqlLoadError> {
    let mut sdl = String::new();

    let types = schema
        .get("types")
        .and_then(|t| t.as_array())
        .ok_or_else(|| {
            GraphqlLoadError::Validation("Introspection schema missing types".to_string())
        })?;

    for type_def in types {
        let name = type_def
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("");
        let kind = type_def
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("");

        // Skip built-in types
        if name.starts_with("__") {
            continue;
        }

        // Skip built-in scalars
        if kind == "SCALAR" && matches!(name, "String" | "Int" | "Float" | "Boolean" | "ID") {
            continue;
        }

        let description = type_def
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");

        if !description.is_empty() {
            sdl.push_str(&format!("\"\"\"{}\"\"\"\n", description));
        }

        match kind {
            "OBJECT" => {
                let interfaces = type_def
                    .get("interfaces")
                    .and_then(|i| i.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|i| i.get("name").and_then(|n| n.as_str()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let implements = if interfaces.is_empty() {
                    String::new()
                } else {
                    format!(" implements {}", interfaces.join(" & "))
                };

                sdl.push_str(&format!("type {}{} {{\n", name, implements));
                emit_fields(&mut sdl, type_def);
                sdl.push_str("}\n\n");
            }
            "INPUT_OBJECT" => {
                sdl.push_str(&format!("input {} {{\n", name));
                if let Some(fields) = type_def.get("inputFields").and_then(|f| f.as_array()) {
                    for field in fields {
                        let fname =
                            field.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let ftype = type_ref_to_sdl(
                            field.get("type").unwrap_or(&serde_json::json!(null)),
                        );
                        sdl.push_str(&format!("  {}: {}\n", fname, ftype));
                    }
                }
                sdl.push_str("}\n\n");
            }
            "INTERFACE" => {
                sdl.push_str(&format!("interface {} {{\n", name));
                emit_fields(&mut sdl, type_def);
                sdl.push_str("}\n\n");
            }
            "ENUM" => {
                sdl.push_str(&format!("enum {} {{\n", name));
                if let Some(vals) = type_def.get("enumValues").and_then(|v| v.as_array()) {
                    for val in vals {
                        let vname =
                            val.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        sdl.push_str(&format!("  {}\n", vname));
                    }
                }
                sdl.push_str("}\n\n");
            }
            "UNION" => {
                let members = type_def
                    .get("possibleTypes")
                    .and_then(|p| p.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                sdl.push_str(&format!(
                    "union {} = {}\n\n",
                    name,
                    members.join(" | ")
                ));
            }
            "SCALAR" => {
                sdl.push_str(&format!("scalar {}\n\n", name));
            }
            _ => {}
        }
    }

    Ok(sdl)
}

/// Emit field definitions for object/interface types.
fn emit_fields(sdl: &mut String, type_def: &serde_json::Value) {
    if let Some(fields) = type_def.get("fields").and_then(|f| f.as_array()) {
        for field in fields {
            let fname = field.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let ftype =
                type_ref_to_sdl(field.get("type").unwrap_or(&serde_json::json!(null)));

            // Emit arguments
            let args = field.get("args").and_then(|a| a.as_array());
            let args_str = if let Some(args) = args {
                if args.is_empty() {
                    String::new()
                } else {
                    let arg_strs: Vec<String> = args
                        .iter()
                        .map(|arg| {
                            let aname =
                                arg.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            let atype = type_ref_to_sdl(
                                arg.get("type").unwrap_or(&serde_json::json!(null)),
                            );
                            format!("{}: {}", aname, atype)
                        })
                        .collect();
                    format!("({})", arg_strs.join(", "))
                }
            } else {
                String::new()
            };

            sdl.push_str(&format!("  {}{}: {}\n", fname, args_str, ftype));
        }
    }
}

/// Convert a `__Type` introspection JSON reference to SDL type notation.
fn type_ref_to_sdl(type_ref: &serde_json::Value) -> String {
    let kind = type_ref
        .get("kind")
        .and_then(|k| k.as_str())
        .unwrap_or("");
    let name = type_ref
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("");

    match kind {
        "NON_NULL" => {
            let inner = type_ref
                .get("ofType")
                .unwrap_or(&serde_json::json!(null));
            format!("{}!", type_ref_to_sdl(inner))
        }
        "LIST" => {
            let inner = type_ref
                .get("ofType")
                .unwrap_or(&serde_json::json!(null));
            format!("[{}]", type_ref_to_sdl(inner))
        }
        _ => name.to_string(),
    }
}
