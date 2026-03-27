//! MCP target -- Model Context Protocol server.
//!
//! Generates a standalone MCP server package that exposes
//! curated GraphQL operations as MCP tools.

use odk_schema::CompiledSchema;
use std::collections::BTreeMap;

/// Generate MCP server files.
pub fn generate(_schema: &CompiledSchema) -> BTreeMap<String, String> {
    BTreeMap::new()
}
