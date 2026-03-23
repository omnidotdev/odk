//! SDK target — vanilla typed GraphQL client.

use odk_schema::CompiledSchema;
use std::collections::BTreeMap;

/// Generate the SDK client.
pub fn generate(_schema: &CompiledSchema) -> BTreeMap<String, String> {
    BTreeMap::new()
}
