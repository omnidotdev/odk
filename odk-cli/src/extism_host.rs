//! Extism Host for loading WASM plugins
//!
//! This module provides functionality to load ODK schema definitions
//! from Extism WASM plugins.

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use extism::{Manifest, Plugin, Wasm};
use odk_schema::CompiledSchema;

/// Load a CompiledSchema from a WASM plugin.
///
/// The plugin must export a `get_schema` function that returns
/// a JSON-serialized CompiledSchema.
pub fn load_schema_from_plugin(plugin_path: &Utf8PathBuf) -> Result<CompiledSchema> {
    // Create manifest from the plugin file
    let wasm = Wasm::file(plugin_path.as_std_path());
    let manifest = Manifest::new([wasm]);

    // Create plugin instance
    let mut plugin = Plugin::new(&manifest, [], true)
        .with_context(|| format!("Failed to load WASM plugin from {}", plugin_path))?;

    // Call the get_schema function
    let result = plugin
        .call::<_, &str>("get_schema", "")
        .with_context(|| "Failed to call get_schema function in plugin")?;

    // Parse the JSON result
    let schema = CompiledSchema::from_json(result)
        .with_context(|| "Failed to parse schema JSON from plugin")?;

    Ok(schema)
}

#[cfg(test)]
mod tests {
    // Tests would go here once we have a test plugin
}
