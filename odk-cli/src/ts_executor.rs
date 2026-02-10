//! Execute TypeScript schemas via bun subprocess.
//!
//! This module compiles TypeScript schema files (using Zod, ArkType, etc.)
//! by spawning a bun subprocess that imports and compiles the schema.

use anyhow::{Context, Result, bail};
use camino::Utf8Path;
use std::process::Command;

use crate::ts_schema_adapter::TsCompiledSchema;

/// Execute a TypeScript schema file and return the compiled schema in TS format.
///
/// This spawns a bun subprocess that:
/// 1. Imports the schema file
/// 2. Compiles it using the ODK TypeScript library
/// 3. Outputs JSON to stdout
pub fn compile_ts_schema(schema_path: &Utf8Path) -> Result<TsCompiledSchema> {
    let schema_path = schema_path
        .canonicalize_utf8()
        .context("Failed to resolve schema path")?;

    // Get ODK TS path from environment or use default
    let odk_ts_path = std::env::var("ODK_TS_PATH")
        .unwrap_or_else(|_| "/home/brian/projects/omni/odk-ts".to_string());

    // Check if bun is available
    if which::which("bun").is_err() {
        bail!(
            "bun not found. Install it with:\n\
             curl -fsSL https://bun.sh/install | bash"
        );
    }

    // Build the inline script to execute
    let script = format!(
        r#"
        import schema from "{}";
        import {{ compile }} from "{}/src/index.ts";
        const compiled = await compile(schema);
        console.log(JSON.stringify(compiled));
        "#,
        schema_path, odk_ts_path
    );

    let output = Command::new("bun")
        .arg("-e")
        .arg(&script)
        .output()
        .context("Failed to execute bun")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Schema compilation failed:\n{}", stderr);
    }

    let stdout = String::from_utf8(output.stdout).context("Invalid UTF-8 in bun output")?;

    let schema: TsCompiledSchema =
        serde_json::from_str(&stdout).context("Failed to parse compiled schema JSON")?;

    Ok(schema)
}
