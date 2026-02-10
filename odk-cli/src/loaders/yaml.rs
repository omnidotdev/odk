//! YAML/JSON schema loader
//!
//! Loads ODK schemas from YAML or JSON files into CompiledSchema.
//!
//! # Example YAML Format
//!
//! ```yaml
//! version: "1.0"
//! name: BlogAPI
//! imports:
//!   - "@typespec/http"
//!   - "@omnidotdev/typespec-drizzle"
//! models:
//!   User:
//!     description: A blog user
//!     decorators: ["@table"]
//!     fields:
//!       id:
//!         type: string
//!         format: uuid
//!         decorators: ["@id"]
//!       email:
//!         type: string
//!         format: email
//! unions:
//!   PostStatus:
//!     variants:
//!       - draft
//!       - published
//! raw: |
//!   @route("/api")
//!   interface BlogApi { }
//! ```

use odk_schema::{
    CompiledField, CompiledModel, CompiledSchema, CompiledType, CompiledUnion,
    CompiledUnionVariant, Extensions, FieldConstraints, LiteralValue, ScalarType,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Error type for YAML loading.
#[derive(Debug)]
pub enum YamlLoadError {
    Io(std::io::Error),
    Parse(serde_yaml::Error),
    Validation(String),
}

impl std::fmt::Display for YamlLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            YamlLoadError::Io(e) => write!(f, "IO error: {}", e),
            YamlLoadError::Parse(e) => write!(f, "YAML parse error: {}", e),
            YamlLoadError::Validation(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for YamlLoadError {}

impl From<std::io::Error> for YamlLoadError {
    fn from(e: std::io::Error) -> Self {
        YamlLoadError::Io(e)
    }
}

impl From<serde_yaml::Error> for YamlLoadError {
    fn from(e: serde_yaml::Error) -> Self {
        YamlLoadError::Parse(e)
    }
}

/// YAML schema file structure.
#[derive(Debug, Deserialize)]
pub struct YamlSchema {
    /// Schema format version
    #[serde(default = "default_version")]
    pub version: String,
    /// Schema name/namespace
    pub name: String,
    /// TypeSpec imports
    #[serde(default)]
    pub imports: Vec<String>,
    /// Model definitions
    #[serde(default)]
    pub models: HashMap<String, YamlModel>,
    /// Union/enum definitions
    #[serde(default)]
    pub unions: HashMap<String, YamlUnion>,
    /// Raw TypeSpec to pass through
    #[serde(default)]
    pub raw: Option<String>,
}

fn default_version() -> String {
    "1.0".to_string()
}

/// YAML model definition.
#[derive(Debug, Deserialize)]
pub struct YamlModel {
    /// Model description
    #[serde(default)]
    pub description: Option<String>,
    /// TypeSpec decorators
    #[serde(default)]
    pub decorators: Vec<String>,
    /// Field definitions
    pub fields: HashMap<String, YamlField>,
}

/// YAML field definition.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum YamlField {
    /// Simple type string (e.g., "string", "int32")
    Simple(String),
    /// Full field definition
    Full(YamlFieldFull),
}

/// Full YAML field definition.
#[derive(Debug, Deserialize)]
pub struct YamlFieldFull {
    /// Field type
    #[serde(rename = "type")]
    pub field_type: String,
    /// Field description
    #[serde(default)]
    pub description: Option<String>,
    /// Whether field is optional
    #[serde(default)]
    pub optional: bool,
    /// Format constraint (uuid, email, etc.)
    #[serde(default)]
    pub format: Option<String>,
    /// TypeSpec decorators
    #[serde(default)]
    pub decorators: Vec<String>,
    /// Validation constraints
    #[serde(default)]
    pub constraints: Option<YamlConstraints>,
}

/// YAML constraint definitions.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YamlConstraints {
    pub min_length: Option<u64>,
    pub max_length: Option<u64>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub pattern: Option<String>,
    pub multiple_of: Option<f64>,
}

/// YAML union definition.
#[derive(Debug, Deserialize)]
pub struct YamlUnion {
    /// Union description
    #[serde(default)]
    pub description: Option<String>,
    /// TypeSpec decorators
    #[serde(default)]
    pub decorators: Vec<String>,
    /// Variant names (for string enums)
    pub variants: Vec<String>,
}

/// Load a schema from a YAML or JSON file.
pub fn load_yaml_schema(path: &Path) -> Result<CompiledSchema, YamlLoadError> {
    let content = std::fs::read_to_string(path)?;
    let file_name = path.to_string_lossy().to_string();

    // Parse based on extension (both use same structure)
    let yaml_schema: YamlSchema = if path.extension().map(|e| e == "json").unwrap_or(false) {
        serde_json::from_str(&content)
            .map_err(|e| YamlLoadError::Validation(format!("JSON parse error: {}", e)))?
    } else {
        serde_yaml::from_str(&content)?
    };

    convert_yaml_to_compiled(yaml_schema, &file_name)
}

/// Load a schema from a YAML string.
pub fn load_yaml_string(content: &str, file_name: &str) -> Result<CompiledSchema, YamlLoadError> {
    let yaml_schema: YamlSchema = serde_yaml::from_str(content)?;
    convert_yaml_to_compiled(yaml_schema, file_name)
}

/// Convert YAML schema to CompiledSchema.
fn convert_yaml_to_compiled(
    yaml: YamlSchema,
    file_name: &str,
) -> Result<CompiledSchema, YamlLoadError> {
    let mut schema = CompiledSchema::new(&yaml.name);

    // Add imports
    for import in yaml.imports {
        schema.add_import(import);
    }

    // Convert unions (do these first as models may reference them)
    for (name, yaml_union) in yaml.unions {
        let union = convert_union(&name, yaml_union, file_name)?;
        schema.add_union(union);
    }

    // Convert models
    for (name, yaml_model) in yaml.models {
        let model = convert_model(&name, yaml_model, file_name)?;
        schema.add_model(model);
    }

    // Add raw TypeSpec
    if let Some(raw) = yaml.raw {
        schema.add_raw(raw);
    }

    Ok(schema)
}

/// Convert a YAML model to CompiledModel.
fn convert_model(
    name: &str,
    yaml: YamlModel,
    file_name: &str,
) -> Result<CompiledModel, YamlLoadError> {
    let mut model = CompiledModel::new(name);
    model.description = yaml.description;

    // Add decorators
    for decorator in yaml.decorators {
        model.extensions.add_decorator(decorator);
    }

    // Convert fields
    for (field_name, yaml_field) in yaml.fields {
        let field = convert_field(&field_name, yaml_field, file_name)?;
        model.add_field(field);
    }

    Ok(model)
}

/// Convert a YAML field to CompiledField.
fn convert_field(
    name: &str,
    yaml: YamlField,
    _file_name: &str,
) -> Result<CompiledField, YamlLoadError> {
    match yaml {
        YamlField::Simple(type_str) => {
            let field_type = parse_type(&type_str)?;
            Ok(CompiledField::new(name, field_type))
        }
        YamlField::Full(full) => {
            let field_type = parse_type(&full.field_type)?;
            let mut field = CompiledField::new(name, field_type);

            if full.optional {
                field.optional = Some(true);
            }

            field.description = full.description;

            // Build constraints
            let mut constraints = FieldConstraints::default();
            let mut has_constraints = false;

            if let Some(format) = full.format {
                constraints.format = Some(format);
                has_constraints = true;
            }

            if let Some(c) = full.constraints {
                constraints.min_length = c.min_length;
                constraints.max_length = c.max_length;
                constraints.min_value = c.min_value;
                constraints.max_value = c.max_value;
                constraints.pattern = c.pattern;
                constraints.multiple_of = c.multiple_of;
                has_constraints = true;
            }

            if has_constraints {
                field.constraints = Some(constraints);
            }

            // Add decorators
            for decorator in full.decorators {
                field.extensions.add_decorator(decorator);
            }

            Ok(field)
        }
    }
}

/// Convert a YAML union to CompiledUnion.
fn convert_union(
    name: &str,
    yaml: YamlUnion,
    _file_name: &str,
) -> Result<CompiledUnion, YamlLoadError> {
    let variants = yaml
        .variants
        .into_iter()
        .map(|v| CompiledUnionVariant {
            name: v.clone(),
            variant_type: Some(CompiledType::Literal {
                value: LiteralValue::String(v),
            }),
        })
        .collect();

    let mut extensions = Extensions::default();
    for decorator in yaml.decorators {
        extensions.add_decorator(decorator);
    }

    Ok(CompiledUnion {
        name: name.to_string(),
        variants,
        description: yaml.description,
        extensions,
        source: None,
    })
}

/// Parse a type string into CompiledType.
fn parse_type(type_str: &str) -> Result<CompiledType, YamlLoadError> {
    let type_str = type_str.trim();

    // Check for array syntax
    if type_str.ends_with("[]") {
        let inner = &type_str[..type_str.len() - 2];
        let inner_type = parse_type(inner)?;
        return Ok(CompiledType::Array {
            items: Box::new(inner_type),
        });
    }

    // Check for scalar types
    match type_str.to_lowercase().as_str() {
        "string" => Ok(CompiledType::Scalar {
            name: ScalarType::String,
        }),
        "int32" | "integer" | "int" => Ok(CompiledType::Scalar {
            name: ScalarType::Int32,
        }),
        "int64" | "bigint" => Ok(CompiledType::Scalar {
            name: ScalarType::Int64,
        }),
        "float32" | "float" => Ok(CompiledType::Scalar {
            name: ScalarType::Float32,
        }),
        "float64" | "double" | "number" => Ok(CompiledType::Scalar {
            name: ScalarType::Float64,
        }),
        "boolean" | "bool" => Ok(CompiledType::Scalar {
            name: ScalarType::Boolean,
        }),
        "date" => Ok(CompiledType::Scalar {
            name: ScalarType::Date,
        }),
        "datetime" | "timestamp" => Ok(CompiledType::Scalar {
            name: ScalarType::Datetime,
        }),
        "bytes" | "binary" => Ok(CompiledType::Scalar {
            name: ScalarType::Bytes,
        }),
        // Otherwise, treat as a reference to another type
        _ => Ok(CompiledType::Ref {
            name: type_str.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_yaml() {
        let yaml = r#"
version: "1.0"
name: TestAPI
models:
  User:
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
"#;

        let schema = load_yaml_string(yaml, "test.yaml").unwrap();
        assert_eq!(schema.name, "TestAPI");
        assert_eq!(schema.models.len(), 1);

        let user = &schema.models[0];
        assert_eq!(user.name, "User");
        assert_eq!(user.fields.len(), 3);
    }

    #[test]
    fn test_parse_with_decorators() {
        let yaml = r#"
name: DrizzleAPI
imports:
  - "@omnidotdev/typespec-drizzle"
models:
  User:
    decorators: ["@table"]
    fields:
      id:
        type: string
        format: uuid
        decorators: ["@id"]
"#;

        let schema = load_yaml_string(yaml, "test.yaml").unwrap();
        assert_eq!(schema.imports.len(), 1);

        let user = &schema.models[0];
        assert_eq!(user.extensions.decorators(), vec!["@table"]);
    }

    #[test]
    fn test_parse_unions() {
        let yaml = r#"
name: EnumAPI
unions:
  Status:
    variants:
      - active
      - inactive
      - pending
"#;

        let schema = load_yaml_string(yaml, "test.yaml").unwrap();
        assert_eq!(schema.unions.len(), 1);

        let status = &schema.unions[0];
        assert_eq!(status.name, "Status");
        assert_eq!(status.variants.len(), 3);
    }

    #[test]
    fn test_parse_raw_block() {
        let yaml = r#"
name: RawAPI
raw: |
  @route("/api")
  interface CustomApi {
    @get op health(): void;
  }
"#;

        let schema = load_yaml_string(yaml, "test.yaml").unwrap();
        assert_eq!(schema.raw_blocks.len(), 1);
        assert!(schema.raw_blocks[0].content.contains("@route"));
    }

    #[test]
    fn test_parse_array_type() {
        let yaml = r#"
name: ArrayAPI
models:
  User:
    fields:
      tags:
        type: string[]
      posts:
        type: Post[]
"#;

        let schema = load_yaml_string(yaml, "test.yaml").unwrap();
        let user = &schema.models[0];

        // Check that fields have array types
        for field in &user.fields {
            match &field.field_type {
                CompiledType::Array { .. } => {}
                _ => panic!("Expected array type for field {}", field.name),
            }
        }
    }
}
