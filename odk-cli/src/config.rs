//! ODK configuration file support
//!
//! Supports loading configuration from:
//! - odk.config.ts (TypeScript, executed via bun)
//! - odk.config.yaml / odk.config.yml
//! - odk.config.json / odk.config.jsonc

use anyhow::{Context, Result, bail};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::process::Command;

/// ODK project configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OdkConfig {
    /// Input schema file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Utf8PathBuf>,

    /// Output directory for generated files
    #[serde(default = "default_output")]
    pub output: Utf8PathBuf,

    /// TypeSpec emitters to use
    #[serde(default)]
    pub emit: Vec<String>,

    /// Emitter-specific options
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,

    /// Namespace for generated TypeSpec
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// Generate configuration (for `odk generate`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate: Option<GenerateConfig>,
}

/// Configuration for the `generate` command.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerateConfig {
    /// Generation targets (e.g. ["sdk", "react-query", "msw", "mcp"])
    #[serde(default)]
    pub targets: Vec<String>,

    /// Output directory for generated files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Utf8PathBuf>,

    /// GraphQL operation documents glob pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operations: Option<String>,

    /// MCP target-specific configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpConfig>,
}

/// MCP server generation configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    /// Server name for the MCP manifest
    pub name: String,

    /// Server description
    #[serde(default)]
    pub description: Option<String>,

    /// Tool definitions mapping GraphQL operations to MCP tools
    #[serde(default)]
    pub tools: Vec<McpToolConfig>,
}

/// A single MCP tool mapped from a GraphQL operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolConfig {
    /// Name of the GraphQL operation (must match a CompiledOperation.name)
    pub operation: String,

    /// MCP tool name (snake_case, used by LLM)
    pub name: String,

    /// Human-readable description of what this tool does
    pub description: String,

    /// Custom parameter definitions with dot-path mappings to GraphQL variables.
    /// When present, the generator produces curated Zod schemas and a variable
    /// builder instead of deriving schemas from the GraphQL operation variables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<McpToolParameter>,
}

/// A curated tool parameter that maps to a nested GraphQL variable path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolParameter {
    /// Parameter name exposed to the agent
    pub name: String,

    /// Base type: "string", "number", or "boolean"
    #[serde(rename = "type")]
    pub type_: String,

    /// Optional format hint: "uuid", "datetime", "email"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    /// Enum values — when non-empty, generates z.enum([...]) instead of base type
    #[serde(default, rename = "enum", skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,

    /// Whether this parameter is required (default: false / optional)
    #[serde(default)]
    pub required: bool,

    /// Default value for the parameter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,

    /// Human-readable description shown to the agent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Dot-path into the GraphQL variables object (e.g. "condition.projectId",
    /// "input.task.content", "filter.dueDate.lessThan")
    #[serde(rename = "mapTo")]
    pub map_to: String,
}

fn default_output() -> Utf8PathBuf {
    Utf8PathBuf::from("./generated")
}

/// Config file names to search for, in priority order
const CONFIG_FILES: &[&str] = &[
    "odk.config.ts",
    "odk.config.yaml",
    "odk.config.yml",
    "odk.config.json",
    "odk.config.jsonc",
];

/// Find a config file in the current directory
pub fn find_config_file() -> Option<Utf8PathBuf> {
    for name in CONFIG_FILES {
        let path = Utf8PathBuf::from(*name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Load configuration from a file
pub fn load_config(path: &Utf8PathBuf) -> Result<OdkConfig> {
    let ext = path.extension().unwrap_or("");

    match ext {
        "ts" => load_ts_config(path),
        "yaml" | "yml" => load_yaml_config(path),
        "json" => load_json_config(path),
        "jsonc" => load_jsonc_config(path),
        _ => bail!("Unsupported config file format: .{}", ext),
    }
}

/// Load TypeScript config by executing via bun
fn load_ts_config(path: &Utf8PathBuf) -> Result<OdkConfig> {
    // Check bun is available
    if which::which("bun").is_err() {
        bail!(
            "bun not found. TypeScript config requires bun.\n\
             Install with: curl -fsSL https://bun.sh/install | bash"
        );
    }

    // Execute the config file and capture JSON output
    let script = format!(
        r#"
        import config from './{path}';
        const resolved = typeof config === 'function' ? config() : config;
        const final = resolved.default ?? resolved;
        console.log(JSON.stringify(final));
        "#,
        path = path
    );

    let output = Command::new("bun")
        .args(["eval", &script])
        .output()
        .context("Failed to execute TypeScript config")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to load TypeScript config:\n{}", stderr);
    }

    let json = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&json).with_context(|| format!("Failed to parse config from {}", path))
}

/// Load YAML config
fn load_yaml_config(path: &Utf8PathBuf) -> Result<OdkConfig> {
    let content = fs::read_to_string(path).with_context(|| format!("Failed to read {}", path))?;

    serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse YAML config from {}", path))
}

/// Load JSON config
fn load_json_config(path: &Utf8PathBuf) -> Result<OdkConfig> {
    let content = fs::read_to_string(path).with_context(|| format!("Failed to read {}", path))?;

    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON config from {}", path))
}

/// Load JSONC config (JSON with comments)
fn load_jsonc_config(path: &Utf8PathBuf) -> Result<OdkConfig> {
    let content = fs::read_to_string(path).with_context(|| format!("Failed to read {}", path))?;

    // Strip comments (simple approach: remove // and /* */ comments)
    let stripped = strip_json_comments(&content);

    serde_json::from_str(&stripped)
        .with_context(|| format!("Failed to parse JSONC config from {}", path))
}

/// Strip JavaScript-style comments from JSON
fn strip_json_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if in_string {
            result.push(c);
            continue;
        }

        if c == '/' {
            match chars.peek() {
                Some('/') => {
                    // Line comment - skip until newline
                    chars.next();
                    while let Some(&next) = chars.peek() {
                        if next == '\n' {
                            break;
                        }
                        chars.next();
                    }
                }
                Some('*') => {
                    // Block comment - skip until */
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => result.push(c),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Merge CLI arguments with config file settings (CLI takes precedence)
pub fn merge_with_cli(
    config: OdkConfig,
    cli_input: Option<&Utf8PathBuf>,
    cli_out: Option<&Utf8PathBuf>,
    cli_emit: &Option<Vec<String>>,
    cli_namespace: &Option<String>,
) -> OdkConfig {
    OdkConfig {
        schema: cli_input.cloned().or(config.schema),
        output: cli_out.cloned().unwrap_or(config.output),
        emit: if let Some(e) = cli_emit {
            if e.is_empty() { config.emit } else { e.clone() }
        } else {
            config.emit
        },
        options: config.options,
        namespace: cli_namespace.clone().or(config.namespace),
        generate: config.generate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_json_comments() {
        let input = r#"{
  // This is a comment
  "key": "value", // inline comment
  /* block
     comment */
  "other": "data"
}"#;

        let stripped = strip_json_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed["key"], "value");
        assert_eq!(parsed["other"], "data");
    }

    #[test]
    fn test_strip_json_comments_in_string() {
        let input = r#"{"url": "https://example.com/path"}"#;
        let stripped = strip_json_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed["url"], "https://example.com/path");
    }
}
