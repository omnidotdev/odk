//! Execute TypeScript schemas via bun subprocess.
//!
//! This module compiles TypeScript schema files (using Zod, ArkType, etc.)
//! by spawning a bun subprocess that imports and compiles the schema.

use anyhow::{Context, Result, bail};
use camino::Utf8Path;
use odk_schema::CompiledSchema;
use std::process::Command;

/// Execute a TypeScript schema file and return the compiled schema.
///
/// This spawns a bun subprocess that:
/// 1. Imports the schema file (which default-exports an `odk.schema(...)` value)
/// 2. Serializes its compiled IR to stdout via `odk.toJSON`
/// 3. Parses that JSON into the canonical `CompiledSchema`
pub fn compile_ts_schema(schema_path: &Utf8Path) -> Result<CompiledSchema> {
    let schema_path = schema_path
        .canonicalize_utf8()
        .context("Failed to resolve schema path")?;

    // Resolve the ODK TypeScript library. By default it is imported as the
    // published package (`@omnidotdev/odk`), resolved from the project's
    // node_modules like any other dependency. ODK_TS_PATH overrides this with a
    // local checkout path for ODK's own development
    let odk_ts_import = std::env::var("ODK_TS_PATH")
        .map(|path| format!("{path}/src/index.ts"))
        .unwrap_or_else(|_| "@omnidotdev/odk".to_string());

    // Check if bun is available
    if which::which("bun").is_err() {
        bail!(
            "bun not found. Install it with:\n\
             curl -fsSL https://bun.sh/install | bash"
        );
    }

    // The schema module default-exports an `odk.schema(...)` value; serialize
    // its compiled IR to stdout for the Rust CLI to consume. Only the JSON is
    // written, so a schema file must not print to stdout itself
    let script = format!(
        r#"
        import schema from "{schema_path}";
        import {{ odk }} from "{odk_ts_import}";
        process.stdout.write(odk.toJSON(schema));
        "#,
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

    let schema: CompiledSchema =
        serde_json::from_str(&stdout).context("Failed to parse compiled schema JSON")?;

    Ok(schema)
}
