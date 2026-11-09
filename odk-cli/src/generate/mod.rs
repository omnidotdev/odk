//! Code generation targets for client artifacts.
//!
//! Generates typed SDK clients, framework-specific hooks, and mock
//! handlers directly from the CompiledSchema IR.

pub mod mcp;
pub mod msw;
pub mod python;
pub mod react_query;
pub mod rust;
pub mod sdk;
pub mod typescript;
pub mod util;

use crate::config::McpGenerateConfig;
use odk_schema::CompiledSchema;
use std::collections::BTreeMap;
use std::fmt;

/// Available generation targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Sdk,
    ReactQuery,
    Msw,
    Mcp,
    Python,
    Typescript,
    Rust,
}

impl Target {
    /// Parse a target name string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sdk" => Some(Target::Sdk),
            "react-query" | "reactquery" => Some(Target::ReactQuery),
            "msw" => Some(Target::Msw),
            "mcp" => Some(Target::Mcp),
            "python" | "py" => Some(Target::Python),
            "typescript" | "ts" => Some(Target::Typescript),
            "rust" | "rs" => Some(Target::Rust),
            _ => None,
        }
    }

    /// Parse a comma-separated list of target names.
    pub fn parse_list(s: &str) -> Result<Vec<Self>, String> {
        let mut targets = Vec::new();
        for part in s.split(',') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            match Self::parse(trimmed) {
                Some(t) => {
                    if !targets.contains(&t) {
                        targets.push(t);
                    }
                }
                None => {
                    return Err(format!(
                        "Unknown target: '{}'. Available: sdk, react-query, msw, mcp, python, typescript, rust",
                        trimmed
                    ));
                }
            }
        }
        Ok(targets)
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Sdk => write!(f, "sdk"),
            Target::ReactQuery => write!(f, "react-query"),
            Target::Msw => write!(f, "msw"),
            Target::Mcp => write!(f, "mcp"),
            Target::Python => write!(f, "python"),
            Target::Typescript => write!(f, "typescript"),
            Target::Rust => write!(f, "rust"),
        }
    }
}

/// Generate client artifacts for the requested targets.
///
/// Returns a map of relative file paths to their contents.
/// The `sdk` target is always included if `react-query` or `msw` is requested.
pub fn generate(
    schema: &CompiledSchema,
    targets: &[Target],
    mcp_config: Option<&McpGenerateConfig>,
) -> BTreeMap<String, String> {
    let mut output = BTreeMap::new();

    // SDK is always generated if any target needs it
    let needs_sdk = targets.contains(&Target::Sdk)
        || targets.contains(&Target::ReactQuery)
        || targets.contains(&Target::Msw);

    if needs_sdk {
        output.extend(sdk::generate(schema));
    }

    if targets.contains(&Target::ReactQuery) {
        output.extend(react_query::generate(schema));
    }

    if targets.contains(&Target::Msw) {
        output.extend(msw::generate(schema));
    }

    if targets.contains(&Target::Python) {
        output.extend(python::generate(schema));
    }

    if targets.contains(&Target::Typescript) {
        output.extend(typescript::generate(schema));
    }

    if targets.contains(&Target::Rust) {
        output.extend(rust::generate(schema));
    }

    if targets.contains(&Target::Mcp) {
        if let Some(cfg) = mcp_config {
            output.extend(mcp::generate_with_config(schema, cfg));
        } else {
            output.extend(mcp::generate(schema));
        }
    }

    output
}

/// Reject a schema a requested target cannot yet emit correctly, before any
/// code is generated.
///
/// Only the `rust` and `typescript` targets emit data-carrying (tagged) unions;
/// the `python` target emits string enums only and would silently drop the
/// payloads. The `rust` target also cannot emit a reference cycle its boxing
/// does not break (mutual recursion), which would produce infinitely-sized Rust.
/// Returning a clear error here turns both silent failures into an actionable
/// message. Targets that handle the schema impose no restriction.
pub fn check_support(schema: &CompiledSchema, targets: &[Target]) -> Result<(), String> {
    if targets.contains(&Target::Python)
        && let Some(union) = schema.unions.iter().find(|u| u.is_data_carrying())
    {
        return Err(format!(
            "the python target does not yet support the data-carrying (tagged) union `{}`; it \
             emits string enums only, so it would drop the variant payloads. Use \
             -t rust,typescript for tagged unions.",
            union.name
        ));
    }

    if targets.contains(&Target::Rust)
        && let Some(message) = rust::unsupported_recursion(schema)
    {
        return Err(message);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use odk_schema::{CompiledType, CompiledUnion, CompiledUnionVariant};

    fn string_enum() -> CompiledUnion {
        CompiledUnion {
            name: "Status".to_string(),
            variants: vec![CompiledUnionVariant::literal_string("draft")],
            ..Default::default()
        }
    }

    fn tagged_union() -> CompiledUnion {
        CompiledUnion {
            name: "OpNode".to_string(),
            tag: Some("op".to_string()),
            variants: vec![CompiledUnionVariant {
                name: "cube".to_string(),
                variant_type: Some(CompiledType::Object { fields: vec![] }),
            }],
            ..Default::default()
        }
    }

    fn schema_with_union(union: CompiledUnion) -> CompiledSchema {
        let mut schema = CompiledSchema::new("S");
        schema.add_union(union);
        schema
    }

    #[test]
    fn python_rejects_a_data_carrying_union() {
        let schema = schema_with_union(tagged_union());
        let err = check_support(&schema, &[Target::Python]).unwrap_err();
        assert!(err.contains("python") && err.contains("OpNode"), "{err}");
    }

    #[test]
    fn python_allows_a_string_enum() {
        let schema = schema_with_union(string_enum());
        assert!(check_support(&schema, &[Target::Python]).is_ok());
    }

    #[test]
    fn typescript_allows_a_data_carrying_union() {
        let schema = schema_with_union(tagged_union());
        assert!(check_support(&schema, &[Target::Typescript]).is_ok());
    }

    #[test]
    fn rust_allows_a_data_carrying_union_without_mutual_recursion() {
        let schema = schema_with_union(tagged_union());
        assert!(check_support(&schema, &[Target::Rust]).is_ok());
    }
}
