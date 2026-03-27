//! Code generation targets for client artifacts.
//!
//! Generates typed SDK clients, framework-specific hooks, and mock
//! handlers directly from the CompiledSchema IR.

pub mod mcp;
pub mod msw;
pub mod react_query;
pub mod sdk;
pub mod util;

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
}

impl Target {
    /// Parse a target name string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sdk" => Some(Target::Sdk),
            "react-query" | "reactquery" => Some(Target::ReactQuery),
            "msw" => Some(Target::Msw),
            "mcp" => Some(Target::Mcp),
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
                None => return Err(format!("Unknown target: '{}'. Available: sdk, react-query, msw, mcp", trimmed)),
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

    if targets.contains(&Target::Mcp) {
        output.extend(mcp::generate(schema));
    }

    output
}
