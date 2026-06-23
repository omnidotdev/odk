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
    /// Generation targets (e.g. ["sdk", "react-query", "msw"])
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
    pub mcp: Option<McpGenerateConfig>,
}

/// MCP target configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpGenerateConfig {
    /// MCP server name (used in package.json and server metadata)
    pub name: String,

    /// Human-readable description of the MCP server
    #[serde(default)]
    pub description: String,

    /// Tools to expose (curated subset of GraphQL operations)
    #[serde(default)]
    pub tools: Vec<McpToolConfig>,
}

/// Configuration for a single MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolConfig {
    /// GraphQL operation name to wrap (must match an operation in the schema)
    pub operation: String,

    /// Tool name override (defaults to snake_case of operation name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Tool description override (defaults to operation description from schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
        .args(["-e", &script])
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

    #[test]
    fn test_mcp_config_deserialization() {
        let json = r#"{
            "schema": "schema.graphql",
            "generate": {
                "targets": ["mcp"],
                "mcp": {
                    "name": "backfeed-mcp",
                    "description": "Backfeed feedback tools",
                    "tools": [
                        {
                            "operation": "createPost",
                            "name": "submit_feedback",
                            "description": "Submit feedback to a board"
                        },
                        {
                            "operation": "posts"
                        }
                    ]
                }
            }
        }"#;

        let config: OdkConfig = serde_json::from_str(json).unwrap();
        let generate = config.generate.unwrap();
        let mcp = generate.mcp.unwrap();
        assert_eq!(mcp.name, "backfeed-mcp");
        assert_eq!(mcp.tools.len(), 2);
        assert_eq!(mcp.tools[0].operation, "createPost");
        assert_eq!(mcp.tools[0].name.as_deref(), Some("submit_feedback"));
        assert_eq!(mcp.tools[1].name, None);
    }
}
