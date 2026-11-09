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

use indexmap::IndexMap;
use odk_schema::{
    CompiledField, CompiledModel, CompiledSchema, CompiledType, CompiledUnion,
    CompiledUnionVariant, Extensions, FieldConstraints, LiteralValue, ScalarType,
};
use serde::Deserialize;
use std::path::Path;

/// Error type for YAML loading.
#[derive(Debug)]
pub enum YamlLoadError {
    Io(std::io::Error),
    Parse(serde_norway::Error),
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

impl From<serde_norway::Error> for YamlLoadError {
    fn from(e: serde_norway::Error) -> Self {
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
    pub models: IndexMap<String, YamlModel>,
    /// Union/enum definitions
    #[serde(default)]
    pub unions: IndexMap<String, YamlUnion>,
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
    pub fields: IndexMap<String, YamlField>,
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
    /// Discriminator key. Its presence marks a data-carrying (tagged) union
    #[serde(default)]
    pub discriminator: Option<String>,
    /// serde `rename_all` rule applied to variant tag values, e.g. "lowercase"
    #[serde(default)]
    pub rename: Option<String>,
    /// Variants: either a list of string names (a plain enum) or a map of
    /// variant name to its payload fields (a data-carrying union)
    pub variants: YamlVariants,
}

/// Union variants in either the string-enum or the data-carrying form.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum YamlVariants {
    /// String enum: a list of variant names
    Strings(Vec<String>),
    /// Data-carrying union: variant name to its payload fields
    Objects(IndexMap<String, IndexMap<String, YamlField>>),
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
        serde_norway::from_str(&content)?
    };

    convert_yaml_to_compiled(yaml_schema, &file_name)
}

/// Load a schema from a YAML string.
pub fn load_yaml_string(content: &str, file_name: &str) -> Result<CompiledSchema, YamlLoadError> {
    let yaml_schema: YamlSchema = serde_norway::from_str(content)?;
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
    file_name: &str,
) -> Result<CompiledUnion, YamlLoadError> {
    let variants = match yaml.variants {
        YamlVariants::Strings(names) => names
            .into_iter()
            .map(|v| CompiledUnionVariant {
                name: v.clone(),
                variant_type: Some(CompiledType::Literal {
                    value: LiteralValue::String(v),
                }),
            })
            .collect(),
        YamlVariants::Objects(variant_map) => {
            let mut variants = Vec::with_capacity(variant_map.len());
            for (variant_name, fields) in variant_map {
                let mut compiled_fields = Vec::with_capacity(fields.len());
                for (field_name, yaml_field) in fields {
                    compiled_fields.push(convert_field(&field_name, yaml_field, file_name)?);
                }
                variants.push(CompiledUnionVariant {
                    name: variant_name,
                    variant_type: Some(CompiledType::Object {
                        fields: compiled_fields,
                    }),
                });
            }
            variants
        }
    };

    let mut extensions = Extensions::default();
    for decorator in yaml.decorators {
        extensions.add_decorator(decorator);
    }

    Ok(CompiledUnion {
        name: name.to_string(),
        variants,
        tag: yaml.discriminator,
        rename_all: yaml.rename,
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
            length: None,
        });
    }

    // Fixed-length array syntax: `base[N]` (e.g. "number[3]" -> [f64; 3])
    if let Some(inner) = type_str.strip_suffix(']') {
        if let Some((base, len_str)) = inner.rsplit_once('[') {
            if let Ok(len) = len_str.parse::<u32>() {
                let inner_type = parse_type(base)?;
                return Ok(CompiledType::Array {
                    items: Box::new(inner_type),
                    length: Some(len),
                });
            }
        }
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
        // TypeSpec built-in temporal scalars, recognized so emitters can act on
        // them (e.g. protobuf, which has no temporal scalar, degrades to string)
        "utcdatetime" => Ok(CompiledType::Scalar {
            name: ScalarType::UtcDateTime,
        }),
        "offsetdatetime" => Ok(CompiledType::Scalar {
            name: ScalarType::OffsetDateTime,
        }),
        "plaindate" => Ok(CompiledType::Scalar {
            name: ScalarType::PlainDate,
        }),
        "plaintime" => Ok(CompiledType::Scalar {
            name: ScalarType::PlainTime,
        }),
        "duration" => Ok(CompiledType::Scalar {
            name: ScalarType::Duration,
        }),
        "url" | "uri" => Ok(CompiledType::Scalar {
            name: ScalarType::Url,
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
    fn test_parse_type_recognizes_builtin_temporal_and_url_scalars() {
        // TypeSpec built-in temporal/url scalar names must load as the matching
        // ScalarType, not fall through to an opaque Ref, so downstream emitters
        // (e.g. protobuf degrading temporal to string) can act on them
        assert!(matches!(
            parse_type("utcDateTime").unwrap(),
            CompiledType::Scalar {
                name: ScalarType::UtcDateTime
            }
        ));
        assert!(matches!(
            parse_type("offsetDateTime").unwrap(),
            CompiledType::Scalar {
                name: ScalarType::OffsetDateTime
            }
        ));
        assert!(matches!(
            parse_type("plainDate").unwrap(),
            CompiledType::Scalar {
                name: ScalarType::PlainDate
            }
        ));
        assert!(matches!(
            parse_type("plainTime").unwrap(),
            CompiledType::Scalar {
                name: ScalarType::PlainTime
            }
        ));
        assert!(matches!(
            parse_type("duration").unwrap(),
            CompiledType::Scalar {
                name: ScalarType::Duration
            }
        ));
        assert!(matches!(
            parse_type("url").unwrap(),
            CompiledType::Scalar {
                name: ScalarType::Url
            }
        ));
    }

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
    fn test_parse_discriminated_union() {
        let yaml = r#"
name: Geo
unions:
  OpNode:
    discriminator: op
    rename: lowercase
    variants:
      cube:
        size: number[3]
        center: boolean
      translate:
        v: number[3]
        child: OpNode
      union:
        children: OpNode[]
"#;

        let schema = load_yaml_string(yaml, "geo.yaml").unwrap();
        let op = schema.unions.iter().find(|u| u.name == "OpNode").unwrap();

        assert_eq!(op.tag.as_deref(), Some("op"));
        assert_eq!(op.rename_all.as_deref(), Some("lowercase"));
        assert_eq!(op.variants.len(), 3);

        let cube = op.variants.iter().find(|v| v.name == "cube").unwrap();
        let Some(CompiledType::Object { fields }) = &cube.variant_type else {
            panic!("cube variant should carry an object payload");
        };
        let size = fields.iter().find(|f| f.name == "size").unwrap();
        assert!(
            matches!(
                &size.field_type,
                CompiledType::Array {
                    length: Some(3),
                    ..
                }
            ),
            "size should be a fixed-length array of 3, got {:?}",
            size.field_type
        );

        let translate = op.variants.iter().find(|v| v.name == "translate").unwrap();
        let Some(CompiledType::Object { fields }) = &translate.variant_type else {
            panic!("translate should carry an object payload");
        };
        let child = fields.iter().find(|f| f.name == "child").unwrap();
        assert!(
            matches!(&child.field_type, CompiledType::Ref { name } if name == "OpNode"),
            "child should be a ref to OpNode"
        );
    }

    #[test]
    fn fields_and_variants_preserve_declaration_order() {
        let yaml = r#"
name: OrderAPI
models:
  Widget:
    fields:
      zeta: string
      alpha: string
      mike: string
      bravo: string
      yankee: string
unions:
  Shape:
    discriminator: kind
    variants:
      square:
        side: number
      circle:
        radius: number
      triangle:
        base: number
"#;
        // load twice: output must be identical (deterministic) and in the
        // order the fields and variants were declared in the YAML
        let first = load_yaml_string(yaml, "order.yaml").unwrap();
        let second = load_yaml_string(yaml, "order.yaml").unwrap();

        let field_names = |s: &CompiledSchema| -> Vec<String> {
            s.models
                .iter()
                .find(|m| m.name == "Widget")
                .unwrap()
                .fields
                .iter()
                .map(|f| f.name.clone())
                .collect()
        };
        assert_eq!(
            field_names(&first),
            ["zeta", "alpha", "mike", "bravo", "yankee"],
            "fields should follow declaration order"
        );
        assert_eq!(
            field_names(&first),
            field_names(&second),
            "load must be deterministic"
        );

        let variant_names: Vec<String> = first
            .unions
            .iter()
            .find(|u| u.name == "Shape")
            .unwrap()
            .variants
            .iter()
            .map(|v| v.name.clone())
            .collect();
        assert_eq!(variant_names, ["square", "circle", "triangle"]);
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
