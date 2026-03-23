//! MSW target — mock handlers and data factories.

use odk_schema::CompiledSchema;
use std::collections::BTreeMap;

/// Generate MSW mock handlers.
pub fn generate(_schema: &CompiledSchema) -> BTreeMap<String, String> {
    BTreeMap::new()
}
