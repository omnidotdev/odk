//! Project templates for `odk init`
//!
//! Provides embedded templates for scaffolding ODK projects in various languages.

/// Available project languages/formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectLang {
    Rust,
    Python,
    Yaml,
}

impl ProjectLang {
    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => Some(Self::Rust),
            "python" | "py" => Some(Self::Python),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }

    /// Display name for prompts
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust (WASM plugin)",
            Self::Python => "Python",
            Self::Yaml => "YAML",
        }
    }

    /// All available languages
    pub fn all() -> &'static [ProjectLang] {
        &[Self::Rust, Self::Python, Self::Yaml]
    }
}

// =============================================================================
// Rust Templates
// =============================================================================

pub const RUST_CARGO_TOML: &str = r#"[package]
name = "{{name}}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
# TODO: Update to crates.io version when published:
# odk-pdk = { version = "0.1", package = "odk-pdk-rust" }
odk-pdk = { git = "https://github.com/omnidotdev/odk", package = "odk-pdk-rust" }
extism-pdk = "1.2"
serde_json = "1.0"
"#;

pub const RUST_LIB_RS: &str = r#"//! {{name}} - ODK Schema Plugin
//!
//! Build: odk build .
//! Compile: odk compile target/odk/{{name}}.wasm -e @typespec/openapi3

use extism_pdk::*;
use odk_pdk::prelude::*;

#[plugin_fn]
pub fn get_schema() -> FnResult<String> {
    let mut schema = CompiledSchema::new("{{name}}");

    // Example model - customize this!
    let mut user = CompiledModel::new("User");
    user.add_field(
        CompiledField::new("id", CompiledType::string())
            .with_format("uuid"),
    );
    user.add_field(
        CompiledField::new("email", CompiledType::string())
            .with_format("email"),
    );
    user.add_field(
        CompiledField::new("name", CompiledType::string())
            .optional(),
    );
    user.add_field(CompiledField::new(
        "createdAt",
        CompiledType::scalar("utcDateTime"),
    ));
    schema.add_model(user);

    Ok(schema.to_json()?)
}
"#;

// =============================================================================
// Python Templates
// =============================================================================

pub const PYTHON_SCHEMA: &str = r#""""
{{name}} - ODK Schema

Usage:
  python schema.py > schema.json
  odk compile schema.json -e @typespec/openapi3
"""

from odk_pdk import Field, Model, Schema

schema = Schema("{{name}}")

# Example model - customize this!
User = Model(
    "User",
    fields=[
        Field("id", type="string", format="uuid"),
        Field("email", type="string", format="email"),
        Field("name", type="string", optional=True),
        Field("createdAt", type="utcDateTime"),
    ],
)

schema.add_models([User])


def get_schema() -> str:
    """Entry point for Extism WASM export (future)."""
    return schema.to_json()


if __name__ == "__main__":
    print(get_schema())
"#;

#[allow(dead_code)]
pub const PYTHON_PYPROJECT: &str = r#"[project]
name = "{{name}}"
version = "0.1.0"
requires-python = ">=3.10"

dependencies = [
    "odk-pdk",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"
"#;

// =============================================================================
// YAML Templates
// =============================================================================

pub const YAML_SCHEMA: &str = r#"# {{name}} - ODK Schema
#
# Compile: odk compile schema.yaml -e @typespec/openapi3

version: "1.0"
name: {{name}}

models:
  User:
    description: A user account
    fields:
      id:
        type: string
        format: uuid
      email:
        type: string
        format: email
      name:
        type: string
        optional: true
      createdAt:
        type: utcDateTime
"#;

// =============================================================================
// Template Rendering
// =============================================================================

/// Render a template by replacing `{{name}}` with the project name
pub fn render(template: &str, name: &str) -> String {
    template.replace("{{name}}", name)
}

/// Post-init instructions for each language
pub fn post_init_message(lang: ProjectLang, name: &str, is_directory: bool) -> String {
    match lang {
        ProjectLang::Rust => {
            format!(
                r#"
Created {name}/ with:
  {name}/Cargo.toml
  {name}/src/lib.rs

Next steps:
  cd {name}
  odk build .
  odk compile target/odk/{name}.wasm -e @typespec/openapi3
"#
            )
        }
        ProjectLang::Python => {
            if is_directory {
                format!(
                    r#"
Created {name}/ with:
  {name}/schema.py
  {name}/pyproject.toml

Next steps:
  cd {name}
  python schema.py > schema.json
  odk compile schema.json -e @typespec/openapi3
"#
                )
            } else {
                r#"
Created schema.py

Next steps:
  python schema.py > schema.json
  odk compile schema.json -e @typespec/openapi3
"#
                .to_string()
            }
        }
        ProjectLang::Yaml => r#"
Created schema.yaml

Next steps:
  odk compile schema.yaml -e @typespec/openapi3
"#
        .to_string(),
    }
}
