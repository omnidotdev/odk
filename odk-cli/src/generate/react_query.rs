//! React Query target — per-operation hooks and provider.

use odk_schema::CompiledSchema;
use std::collections::BTreeMap;

/// Generate React Query hooks.
pub fn generate(_schema: &CompiledSchema) -> BTreeMap<String, String> {
    BTreeMap::new()
}
