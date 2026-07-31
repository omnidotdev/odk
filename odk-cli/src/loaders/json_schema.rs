//! JSON Schema loader
//!
//! Loads a standard [JSON Schema](https://json-schema.org) document into a
//! `CompiledSchema`, so schemas that already exist elsewhere become ODK inputs
//! without being rewritten in ODK's own format. The motivating case is Rust
//! types that derive their schema via `schemars` (for example Resonance's
//! `ProjectDocument`), where the Rust type is the source of truth and the JSON
//! Schema is generated from it.
//!
//! Draft 2020-12 (`$defs`) and draft-07 (`definitions`) are both accepted; they
//! differ here only in where named subschemas live.
//!
//! # Mapping
//!
//! | JSON Schema                        | CompiledSchema                     |
//! |------------------------------------|------------------------------------|
//! | `title`                            | schema name (falls back to file stem) |
//! | root object with `properties`      | a model named after the title      |
//! | `$defs` / `definitions` entry      | a model, or a union if it is a string `enum` |
//! | `properties.X`                     | a field, optional unless listed in `required` |
//! | `$ref: "#/$defs/Foo"`              | `CompiledType::Ref { name: "Foo" }`|
//! | `type: [T, "null"]`                | type `T`, field marked optional    |
//! | `oneOf` / `anyOf`                  | `CompiledType::Union`              |
//! | `items`                            | `CompiledType::Array`              |
//! | inline `type: object`              | `CompiledType::Object`             |
//! | `minLength`, `maximum`, `pattern`, … | `FieldConstraints`               |
//!
//! # Not represented
//!
//! Keywords with no ODK equivalent are dropped rather than approximated:
//! multi-branch `allOf` composition, `not`, conditional `if`/`then`/`else`,
//! `patternProperties`, and `additionalProperties` schemas. A schema relying on
//! them loads, but the constraint is not carried into the IR.
//!
//! A *single-branch* `allOf` is unwrapped, because that is how schemars emits a
//! `$ref` on a field that also has a description, and treating it as
//! unrepresentable would silently drop the reference.

use std::collections::HashMap;
use std::path::Path;

use odk_schema::{
    CompiledField, CompiledModel, CompiledSchema, CompiledType, CompiledUnion,
    CompiledUnionVariant, FieldConstraints, LiteralValue, ScalarType,
};
use serde_json::Value;

/// Error type for JSON Schema loading.
#[derive(Debug)]
pub enum JsonSchemaLoadError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    Validation(String),
}

impl std::fmt::Display for JsonSchemaLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonSchemaLoadError::Io(e) => write!(f, "IO error: {}", e),
            JsonSchemaLoadError::Parse(e) => write!(f, "JSON parse error: {}", e),
            JsonSchemaLoadError::Validation(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for JsonSchemaLoadError {}

impl From<std::io::Error> for JsonSchemaLoadError {
    fn from(e: std::io::Error) -> Self {
        JsonSchemaLoadError::Io(e)
    }
}

impl From<serde_json::Error> for JsonSchemaLoadError {
    fn from(e: serde_json::Error) -> Self {
        JsonSchemaLoadError::Parse(e)
    }
}

/// Whether a JSON document looks like a JSON Schema rather than an ODK schema.
///
/// ODK's own JSON format is keyed by `models`, so that wins if both are somehow
/// present: this only claims documents that are unambiguously JSON Schema.
pub fn is_json_schema(content: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        return false;
    };
    let Some(obj) = value.as_object() else {
        return false;
    };

    if obj.contains_key("models") {
        return false;
    }

    let declared = obj
        .get("$schema")
        .and_then(Value::as_str)
        .is_some_and(|s| s.contains("json-schema.org"));

    declared
        || obj.contains_key("$defs")
        || obj.contains_key("definitions")
        || (obj.contains_key("properties")
            && obj.get("type").and_then(Value::as_str) == Some("object"))
}

/// Load a JSON Schema document from a file.
///
/// # Errors
///
/// Returns an error if the file cannot be read, is not valid JSON, or is not an
/// object at the root.
pub fn load_json_schema(path: &Path) -> Result<CompiledSchema, JsonSchemaLoadError> {
    let content = std::fs::read_to_string(path)?;
    let fallback_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(sanitize_name)
        .unwrap_or_else(|| "Schema".to_string());

    parse_json_schema(&content, &fallback_name)
}

/// Convert a JSON Schema document into a `CompiledSchema`.
///
/// # Errors
///
/// Returns an error if the document is not valid JSON or is not a JSON object.
pub fn parse_json_schema(
    content: &str,
    fallback_name: &str,
) -> Result<CompiledSchema, JsonSchemaLoadError> {
    let root: Value = serde_json::from_str(content)?;
    let obj = root
        .as_object()
        .ok_or_else(|| JsonSchemaLoadError::Validation("root must be a JSON object".into()))?;

    let title = obj.get("title").and_then(Value::as_str);
    let name = title.map_or_else(|| fallback_name.to_string(), sanitize_name);

    let mut schema = CompiledSchema::new(&name);

    // Named subschemas: 2020-12 puts them in $defs, draft-07 in definitions
    let defs = obj
        .get("$defs")
        .or_else(|| obj.get("definitions"))
        .and_then(Value::as_object);

    if let Some(defs) = defs {
        for (def_name, def) in defs {
            let def_name = sanitize_name(def_name);

            if let Some(union) = string_enum_union(&def_name, def) {
                schema.add_union(union);
            } else {
                schema.add_model(object_model(&def_name, def));
            }
        }
    }

    // The root itself is a model when it describes an object. A schema that is
    // only a container for $defs (no properties of its own) contributes none
    if obj.contains_key("properties") {
        schema.add_model(object_model(&name, &root));
    }

    Ok(schema)
}

/// Build a model from an object subschema.
fn object_model(name: &str, schema: &Value) -> CompiledModel {
    let mut model = CompiledModel::new(name);

    if let Some(desc) = schema.get("description").and_then(Value::as_str) {
        model.description = Some(desc.to_string());
    }

    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        for (prop_name, prop) in props {
            let (field_type, nullable) = convert_type(prop);

            let mut field = CompiledField::new(prop_name, field_type);

            // A property is optional if it is absent from `required`, or if its
            // type admits null. schemars emits both shapes depending on whether
            // the Rust field is `Option<T>` or carries `#[serde(default)]`
            if nullable || !required.contains(&prop_name.as_str()) {
                field = field.optional();
            }

            if let Some(desc) = prop.get("description").and_then(Value::as_str) {
                field.description = Some(desc.to_string());
            }

            field.constraints = constraints(prop);

            model.add_field(field);
        }
    }

    model
}

/// Build a union from a subschema whose `enum` is a list of strings.
fn string_enum_union(name: &str, schema: &Value) -> Option<CompiledUnion> {
    let values = schema.get("enum")?.as_array()?;

    let variants: Vec<CompiledUnionVariant> = values
        .iter()
        .filter_map(Value::as_str)
        .map(|v| CompiledUnionVariant {
            name: v.to_string(),
            // the literal is what makes this a value union in TypeSpec; without
            // it the variant name is emitted as a bare identifier and fails to
            // resolve
            variant_type: Some(CompiledType::Literal {
                value: LiteralValue::String(v.to_string()),
            }),
        })
        .collect();

    // Mixed-type enums are not string unions; leave them to the type mapper
    if variants.len() != values.len() || variants.is_empty() {
        return None;
    }

    Some(CompiledUnion {
        name: name.to_string(),
        variants,
        description: schema
            .get("description")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        extensions: odk_schema::Extensions::default(),
        source: None,
    })
}

/// Convert a subschema to a type, reporting whether it also admits null.
fn convert_type(schema: &Value) -> (CompiledType, bool) {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return (
            CompiledType::Ref {
                name: ref_name(reference),
            },
            false,
        );
    }

    // schemars wraps a $ref in a single-branch allOf whenever the property also
    // carries a description, so unwrapping that is what keeps references from
    // being silently flattened to an empty type
    if let Some([only]) = schema
        .get("allOf")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    {
        return convert_type(only);
    }

    // oneOf/anyOf: a single non-null branch is the common `Option<T>` shape and
    // collapses to that branch; anything else stays a union
    for key in ["oneOf", "anyOf"] {
        if let Some(branches) = schema.get(key).and_then(Value::as_array) {
            let non_null: Vec<&Value> = branches.iter().filter(|b| !is_null_schema(b)).collect();
            let nullable = non_null.len() < branches.len();

            if non_null.len() == 1 {
                let (inner, inner_nullable) = convert_type(non_null[0]);
                return (inner, nullable || inner_nullable);
            }

            let variants: Vec<CompiledType> = non_null.iter().map(|b| convert_type(b).0).collect();

            if !variants.is_empty() {
                return (CompiledType::Union { variants }, nullable);
            }
        }
    }

    // `type` may be a string or a list including "null"
    let (type_name, nullable) = match schema.get("type") {
        Some(Value::String(s)) => (Some(s.as_str()), false),
        Some(Value::Array(types)) => {
            let nullable = types.iter().any(|t| t.as_str() == Some("null"));
            let first = types
                .iter()
                .filter_map(Value::as_str)
                .find(|t| *t != "null");
            (first, nullable)
        }
        _ => (None, false),
    };

    let format = schema.get("format").and_then(Value::as_str);

    let compiled = match type_name {
        Some("string") => CompiledType::Scalar {
            name: string_scalar(format),
        },
        Some("integer") => CompiledType::Scalar {
            name: integer_scalar(format),
        },
        Some("number") => CompiledType::Scalar {
            name: match format {
                Some("float") => ScalarType::Float32,
                _ => ScalarType::Float64,
            },
        },
        Some("boolean") => CompiledType::Scalar {
            name: ScalarType::Boolean,
        },
        Some("array") => {
            let items = schema.get("items").map_or(
                CompiledType::Scalar {
                    name: ScalarType::Unknown,
                },
                |i| convert_type(i).0,
            );

            CompiledType::Array {
                items: Box::new(items),
            }
        }
        Some("object") => {
            // An inline object with properties becomes a structural type; a bare
            // `type: object` (free-form map) has nothing to describe
            if schema.get("properties").is_some() {
                let model = object_model("", schema);
                CompiledType::Object {
                    fields: model.fields,
                }
            } else {
                CompiledType::Scalar {
                    name: ScalarType::Unknown,
                }
            }
        }
        _ => CompiledType::Scalar {
            name: ScalarType::Unknown,
        },
    };

    (compiled, nullable)
}

/// Map a string `format` to the narrowest scalar ODK has.
fn string_scalar(format: Option<&str>) -> ScalarType {
    match format {
        Some("date-time") => ScalarType::UtcDateTime,
        Some("date") => ScalarType::PlainDate,
        Some("time") => ScalarType::PlainTime,
        Some("duration") => ScalarType::Duration,
        Some("uri" | "url") => ScalarType::Url,
        Some("byte" | "binary") => ScalarType::Bytes,
        // uuid and email stay strings; the format is preserved in constraints
        _ => ScalarType::String,
    }
}

/// Map an integer `format` to a width. JSON Schema does not bound integers by
/// default, so the wider type is the safe assumption.
fn integer_scalar(format: Option<&str>) -> ScalarType {
    match format {
        Some("int32" | "uint32") => ScalarType::Int32,
        _ => ScalarType::Int64,
    }
}

/// Extract the trailing name from a local `$ref` pointer.
fn ref_name(reference: &str) -> String {
    reference
        .rsplit('/')
        .next()
        .map_or_else(|| sanitize_name(reference), sanitize_name)
}

/// Whether a subschema is exactly the null type.
fn is_null_schema(schema: &Value) -> bool {
    schema.get("type").and_then(Value::as_str) == Some("null")
}

/// Collect validation keywords ODK can represent.
fn constraints(schema: &Value) -> Option<FieldConstraints> {
    let num = |key: &str| schema.get(key).and_then(Value::as_f64);
    let uint = |key: &str| schema.get(key).and_then(Value::as_u64);

    // `format` is only carried for strings: TypeSpec's @format targets strings,
    // and a numeric format (schemars stamps uint32, double, and friends) has
    // already been folded into the scalar choice
    let is_string = match schema.get("type") {
        Some(Value::String(s)) => s == "string",
        Some(Value::Array(types)) => types.iter().any(|t| t.as_str() == Some("string")),
        _ => false,
    };

    let format = schema
        .get("format")
        .and_then(Value::as_str)
        .filter(|_| is_string)
        // formats that became a scalar carry no extra information here
        .filter(|f| {
            !matches!(
                *f,
                "date-time" | "date" | "time" | "duration" | "uri" | "url" | "byte" | "binary"
            )
        })
        .map(ToString::to_string);

    let constraints = FieldConstraints {
        min_length: uint("minLength"),
        max_length: uint("maxLength"),
        min_value: num("minimum"),
        max_value: num("maximum"),
        pattern: schema
            .get("pattern")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        format,
        multiple_of: num("multipleOf"),
    };

    let empty = constraints.min_length.is_none()
        && constraints.max_length.is_none()
        && constraints.min_value.is_none()
        && constraints.max_value.is_none()
        && constraints.pattern.is_none()
        && constraints.format.is_none()
        && constraints.multiple_of.is_none();

    if empty { None } else { Some(constraints) }
}

/// Make a name safe to emit as a TypeSpec identifier.
fn sanitize_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();

    // An identifier cannot lead with a digit
    if cleaned.chars().next().is_some_and(char::is_numeric) {
        format!("_{cleaned}")
    } else {
        cleaned
    }
}

/// Named lookup of a schema's models, for tests and callers that want one.
#[allow(dead_code)]
pub fn models_by_name(schema: &CompiledSchema) -> HashMap<&str, &CompiledModel> {
    schema.models.iter().map(|m| (m.name.as_str(), m)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(json: &str) -> CompiledSchema {
        parse_json_schema(json, "Fallback").expect("schema parses")
    }

    fn field<'a>(model: &'a CompiledModel, name: &str) -> &'a CompiledField {
        model
            .fields
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("field {name} missing"))
    }

    #[test]
    fn detects_json_schema_and_rejects_odk_schema() {
        assert!(is_json_schema(
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{}}"#
        ));
        assert!(is_json_schema(r#"{"$defs":{"A":{"type":"object"}}}"#));
        assert!(is_json_schema(
            r#"{"type":"object","properties":{"a":{"type":"string"}}}"#
        ));

        // ODK's own format must keep winning
        assert!(!is_json_schema(
            r#"{"name":"API","models":{"User":{"fields":{}}}}"#
        ));
        assert!(!is_json_schema("not json"));
        assert!(!is_json_schema("[]"));
    }

    #[test]
    fn root_object_becomes_a_model_named_after_the_title() {
        let schema = load(
            r#"{
              "title": "ProjectDocument",
              "type": "object",
              "properties": { "name": { "type": "string" } },
              "required": ["name"]
            }"#,
        );

        assert_eq!(schema.name, "ProjectDocument");
        assert_eq!(schema.models.len(), 1);
        assert_eq!(schema.models[0].name, "ProjectDocument");
    }

    #[test]
    fn falls_back_to_the_given_name_when_untitled() {
        let schema = load(r#"{"type":"object","properties":{"a":{"type":"string"}}}"#);
        assert_eq!(schema.name, "Fallback");
    }

    #[test]
    fn required_drives_optionality() {
        let schema = load(
            r#"{
              "title": "T", "type": "object",
              "properties": { "a": { "type": "string" }, "b": { "type": "string" } },
              "required": ["a"]
            }"#,
        );
        let model = &schema.models[0];

        assert_eq!(field(model, "a").optional, None);
        assert_eq!(field(model, "b").optional, Some(true));
    }

    #[test]
    fn a_nullable_type_is_optional_even_when_required() {
        let schema = load(
            r#"{
              "title": "T", "type": "object",
              "properties": { "a": { "type": ["string", "null"] } },
              "required": ["a"]
            }"#,
        );

        let a = field(&schema.models[0], "a");
        assert_eq!(a.optional, Some(true));
        assert!(matches!(
            a.field_type,
            CompiledType::Scalar {
                name: ScalarType::String
            }
        ));
    }

    #[test]
    fn scalars_and_formats_map_to_the_narrowest_type() {
        let schema = load(
            r#"{
              "title": "T", "type": "object",
              "properties": {
                "s": { "type": "string" },
                "stamp": { "type": "string", "format": "date-time" },
                "id": { "type": "string", "format": "uuid" },
                "i": { "type": "integer" },
                "i32": { "type": "integer", "format": "int32" },
                "f": { "type": "number" },
                "b": { "type": "boolean" }
              }
            }"#,
        );
        let m = &schema.models[0];

        assert!(matches!(
            field(m, "s").field_type,
            CompiledType::Scalar {
                name: ScalarType::String
            }
        ));
        assert!(matches!(
            field(m, "stamp").field_type,
            CompiledType::Scalar {
                name: ScalarType::UtcDateTime
            }
        ));
        assert!(matches!(
            field(m, "i").field_type,
            CompiledType::Scalar {
                name: ScalarType::Int64
            }
        ));
        assert!(matches!(
            field(m, "i32").field_type,
            CompiledType::Scalar {
                name: ScalarType::Int32
            }
        ));
        assert!(matches!(
            field(m, "f").field_type,
            CompiledType::Scalar {
                name: ScalarType::Float64
            }
        ));
        assert!(matches!(
            field(m, "b").field_type,
            CompiledType::Scalar {
                name: ScalarType::Boolean
            }
        ));

        // uuid stays a string but keeps the format as a constraint
        let id = field(m, "id");
        assert!(matches!(
            id.field_type,
            CompiledType::Scalar {
                name: ScalarType::String
            }
        ));
        assert_eq!(
            id.constraints.as_ref().unwrap().format.as_deref(),
            Some("uuid")
        );
    }

    #[test]
    fn refs_resolve_to_the_definition_name() {
        let schema = load(
            r##"{
              "title": "T", "type": "object",
              "properties": { "track": { "$ref": "#/$defs/TrackManifest" } },
              "$defs": { "TrackManifest": { "type": "object", "properties": { "id": { "type": "string" } } } }
            }"##,
        );

        let root = schema.models.iter().find(|m| m.name == "T").unwrap();
        assert!(matches!(
            &field(root, "track").field_type,
            CompiledType::Ref { name } if name == "TrackManifest"
        ));
        assert!(schema.models.iter().any(|m| m.name == "TrackManifest"));
    }

    #[test]
    fn a_single_branch_all_of_unwraps_to_its_ref() {
        // the shape schemars emits for a described $ref field
        let schema = load(
            r##"{
              "title": "T", "type": "object",
              "properties": {
                "project": {
                  "description": "the manifest",
                  "allOf": [{ "$ref": "#/definitions/ProjectManifest" }]
                }
              },
              "definitions": { "ProjectManifest": { "type": "object", "properties": {} } }
            }"##,
        );

        let root = schema.models.iter().find(|m| m.name == "T").unwrap();
        let project = field(root, "project");
        assert!(matches!(
            &project.field_type,
            CompiledType::Ref { name } if name == "ProjectManifest"
        ));
        assert_eq!(project.description.as_deref(), Some("the manifest"));
    }

    #[test]
    fn arrays_carry_their_item_type() {
        let schema = load(
            r##"{
              "title": "T", "type": "object",
              "properties": { "tracks": { "type": "array", "items": { "$ref": "#/$defs/Track" } } },
              "$defs": { "Track": { "type": "object", "properties": {} } }
            }"##,
        );

        let root = schema.models.iter().find(|m| m.name == "T").unwrap();
        let CompiledType::Array { items } = &field(root, "tracks").field_type else {
            panic!("expected an array");
        };
        assert!(matches!(items.as_ref(), CompiledType::Ref { name } if name == "Track"));
    }

    #[test]
    fn a_string_enum_definition_becomes_a_union() {
        let schema = load(
            r#"{
              "title": "T",
              "$defs": { "TrackType": { "enum": ["audio", "midi", "bus", "master"] } }
            }"#,
        );

        assert_eq!(schema.unions.len(), 1);
        let union = &schema.unions[0];
        assert_eq!(union.name, "TrackType");
        let names: Vec<&str> = union.variants.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["audio", "midi", "bus", "master"]);
    }

    #[test]
    fn option_shaped_one_of_collapses_to_its_branch() {
        let schema = load(
            r#"{
              "title": "T", "type": "object",
              "properties": { "region": { "oneOf": [{ "type": "string" }, { "type": "null" }] } },
              "required": ["region"]
            }"#,
        );

        let region = field(&schema.models[0], "region");
        assert_eq!(region.optional, Some(true));
        assert!(matches!(
            region.field_type,
            CompiledType::Scalar {
                name: ScalarType::String
            }
        ));
    }

    #[test]
    fn a_multi_branch_one_of_stays_a_union() {
        let schema = load(
            r#"{
              "title": "T", "type": "object",
              "properties": { "v": { "oneOf": [{ "type": "string" }, { "type": "integer" }] } }
            }"#,
        );

        let CompiledType::Union { variants } = &field(&schema.models[0], "v").field_type else {
            panic!("expected a union");
        };
        assert_eq!(variants.len(), 2);
    }

    #[test]
    fn inline_objects_become_structural_types() {
        let schema = load(
            r#"{
              "title": "T", "type": "object",
              "properties": {
                "point": {
                  "type": "object",
                  "properties": { "x": { "type": "number" }, "y": { "type": "number" } },
                  "required": ["x", "y"]
                }
              }
            }"#,
        );

        let CompiledType::Object { fields } = &field(&schema.models[0], "point").field_type else {
            panic!("expected an inline object");
        };
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn validation_keywords_become_constraints() {
        let schema = load(
            r#"{
              "title": "T", "type": "object",
              "properties": {
                "tempo": { "type": "number", "minimum": 20.0, "maximum": 300.0 },
                "name": { "type": "string", "minLength": 1, "maxLength": 64, "pattern": "^[a-z]+$" }
              }
            }"#,
        );
        let m = &schema.models[0];

        let tempo = field(m, "tempo").constraints.as_ref().unwrap();
        assert_eq!(tempo.min_value, Some(20.0));
        assert_eq!(tempo.max_value, Some(300.0));

        let name = field(m, "name").constraints.as_ref().unwrap();
        assert_eq!(name.min_length, Some(1));
        assert_eq!(name.max_length, Some(64));
        assert_eq!(name.pattern.as_deref(), Some("^[a-z]+$"));

        // a field with no validation keywords carries no constraints at all
        assert!(field(m, "tempo").description.is_none());
    }

    #[test]
    fn draft_07_definitions_are_read_like_defs() {
        let schema = load(
            r#"{
              "title": "T",
              "definitions": { "Marker": { "type": "object", "properties": { "id": { "type": "string" } } } }
            }"#,
        );

        assert!(schema.models.iter().any(|m| m.name == "Marker"));
    }

    #[test]
    fn descriptions_carry_through() {
        let schema = load(
            r#"{
              "title": "T", "type": "object", "description": "A project",
              "properties": { "a": { "type": "string", "description": "the a" } }
            }"#,
        );

        assert_eq!(schema.models[0].description.as_deref(), Some("A project"));
        assert_eq!(
            field(&schema.models[0], "a").description.as_deref(),
            Some("the a")
        );
    }

    #[test]
    fn names_that_are_not_identifiers_are_sanitized() {
        let schema = load(r#"{"$defs": { "foo-bar": { "type": "object", "properties": {} } }}"#);
        assert!(schema.models.iter().any(|m| m.name == "foo_bar"));

        assert_eq!(sanitize_name("2fast"), "_2fast");
    }

    #[test]
    fn a_defs_only_document_contributes_no_root_model() {
        let schema = load(r#"{"title": "T", "$defs": {"A": {"type":"object","properties":{}}}}"#);

        assert_eq!(schema.models.len(), 1);
        assert_eq!(schema.models[0].name, "A");
    }

    #[test]
    fn a_non_object_root_is_rejected() {
        assert!(parse_json_schema("[]", "X").is_err());
        assert!(parse_json_schema("{", "X").is_err());
    }
}
