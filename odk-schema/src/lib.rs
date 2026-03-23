//! ODK Schema Types
//!
//! This crate defines the CompiledSchema IR (intermediate representation)
//! that all ODK schema sources compile to. This is the same format used
//! by the TypeScript SDK.
//!
//! # Architecture
//!
//! ```text
//! Input Sources (TS SDK, YAML, WASM)
//!         │
//!         ▼
//!    CompiledSchema IR  ◄── This crate
//!         │
//!         ▼
//!    TypeSpec Bridge
//!         │
//!         ▼
//!    TypeSpec Emitters
//! ```
//!
//! # Features
//!
//! - **Extensions**: Pass through Zod transforms, custom decorators, etc.
//! - **Source Maps**: Track original source locations for error messages
//! - **Raw Blocks**: Escape hatch for arbitrary TypeSpec code

use serde::{Deserialize, Serialize};

pub mod extensions;
pub mod source_map;

pub use extensions::{Extensions, KnownExtension};
pub use source_map::{RawTypeSpecBlock, SourceMap, SourceSpan};

/// A compiled schema containing models, unions, and operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledSchema {
    /// Name of the schema namespace
    pub name: String,
    /// Model definitions
    pub models: Vec<CompiledModel>,
    /// Union/enum definitions
    pub unions: Vec<CompiledUnion>,
    /// Operation definitions (queries, mutations, endpoints, RPCs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<CompiledOperation>,
    /// Fragment definitions used by operations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fragments: Vec<FragmentDefinition>,
    /// Raw TypeSpec blocks to pass through verbatim
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_blocks: Vec<RawTypeSpecBlock>,
    /// TypeSpec imports required by this schema
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    /// Warnings generated during compilation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

/// A compiled operation representing something a client can do.
///
/// Covers GraphQL queries/mutations/subscriptions, REST endpoints, and gRPC RPCs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledOperation {
    /// Operation name (e.g. "getUser", "createPost")
    pub name: String,
    /// What kind of operation this is
    pub kind: OperationKind,
    /// Input/argument type (None for no-arg operations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<CompiledType>,
    /// Return type
    pub output: CompiledType,
    /// Variable declarations for this operation
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<Variable>,
    /// Field selection set (for GraphQL operations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<SelectionSet>,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Decorators and extensions for this operation
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Directives applied to this operation
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives: Vec<Directive>,
}

/// The kind of operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    /// GraphQL query
    Query,
    /// GraphQL mutation
    Mutation,
    /// GraphQL subscription
    Subscription,
    /// gRPC RPC
    Rpc,
    /// REST endpoint
    Endpoint,
}

/// A set of selected fields within an operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionSet {
    /// Selected fields
    pub fields: Vec<FieldSelection>,
    /// Fragment spreads (e.g. `...UserFields`)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fragment_spreads: Vec<FragmentSpread>,
    /// Inline fragments (e.g. `... on Admin { role }`)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inline_fragments: Vec<InlineFragment>,
}

/// A single field selection within a selection set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSelection {
    /// Field name
    pub name: String,
    /// Optional alias for this field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Arguments passed to this field
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<(String, CompiledType)>,
    /// Nested selection set
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<SelectionSet>,
    /// Directives applied to this field
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives: Vec<Directive>,
}

/// A directive applied to a field selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Directive {
    /// Directive name (without @)
    pub name: String,
    /// Directive arguments as key-value pairs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<(String, serde_json::Value)>,
}

/// A fragment spread within a selection set (e.g. `...UserFields`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentSpread {
    /// Fragment name
    pub name: String,
    /// Directives applied to this spread
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives: Vec<Directive>,
}

/// An inline fragment within a selection set (e.g. `... on Admin { role }`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineFragment {
    /// Type condition (None for unconditional inline fragments)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_condition: Option<String>,
    /// Selection set within this fragment
    pub selection: SelectionSet,
    /// Directives applied to this fragment
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives: Vec<Directive>,
}

/// A named fragment definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentDefinition {
    /// Fragment name
    pub name: String,
    /// Type condition (the type this fragment applies to)
    pub type_condition: String,
    /// Selection set
    pub selection: SelectionSet,
    /// Directives applied to this fragment
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives: Vec<Directive>,
}

/// A variable declaration in an operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    /// Variable name (without $)
    pub name: String,
    /// Variable type
    #[serde(rename = "type")]
    pub variable_type: CompiledType,
    /// Whether the variable is required (non-nullable in GraphQL)
    #[serde(default)]
    pub required: bool,
    /// Default value (JSON representation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A compiled model (object type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledModel {
    /// Model name
    pub name: String,
    /// Field definitions
    pub fields: Vec<CompiledField>,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Decorators and extensions for this model
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Source location for error mapping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceSpan>,
}

/// A compiled field within a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledField {
    /// Field name
    pub name: String,
    /// Field type
    #[serde(rename = "type")]
    pub field_type: CompiledType,
    /// Whether the field is optional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Field constraints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<FieldConstraints>,
    /// Decorators and extensions for this field
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Source location for error mapping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceSpan>,
}

/// Field validation constraints.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldConstraints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiple_of: Option<f64>,
}

/// A type reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CompiledType {
    /// Scalar type (string, int32, etc.)
    Scalar { name: ScalarType },
    /// Array type
    Array { items: Box<CompiledType> },
    /// Reference to another type
    Ref { name: String },
    /// Literal value
    Literal { value: LiteralValue },
    /// Union of types
    Union { variants: Vec<CompiledType> },
    /// Inline object
    Object { fields: Vec<CompiledField> },
}

/// Scalar type names.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScalarType {
    String,
    Int32,
    Int64,
    Float32,
    Float64,
    Boolean,
    Date,
    Datetime,
    /// TypeSpec utcDateTime - datetime with UTC timezone
    UtcDateTime,
    /// TypeSpec offsetDateTime - datetime with timezone offset
    OffsetDateTime,
    /// TypeSpec plainDate - date without time
    PlainDate,
    /// TypeSpec plainTime - time without date
    PlainTime,
    /// TypeSpec duration - ISO 8601 duration
    Duration,
    Bytes,
    /// TypeSpec url - URL string
    Url,
    Unknown,
}

/// Literal values for const types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LiteralValue {
    String(String),
    Number(f64),
    Bool(bool),
}

/// A compiled union/enum type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompiledUnion {
    /// Union name
    pub name: String,
    /// Variant definitions
    pub variants: Vec<CompiledUnionVariant>,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Decorators and extensions for this union
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Source location for error mapping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceSpan>,
}

/// A variant within a union.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledUnionVariant {
    /// Variant name
    pub name: String,
    /// Optional type (for discriminated unions)
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub variant_type: Option<CompiledType>,
}

impl CompiledSchema {
    /// Create a new empty schema.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            models: Vec::new(),
            unions: Vec::new(),
            operations: Vec::new(),
            fragments: Vec::new(),
            raw_blocks: Vec::new(),
            imports: Vec::new(),
            warnings: None,
        }
    }

    /// Add a model to the schema.
    pub fn add_model(&mut self, model: CompiledModel) {
        self.models.push(model);
    }

    /// Add a union to the schema.
    pub fn add_union(&mut self, union: CompiledUnion) {
        self.unions.push(union);
    }

    /// Add an operation to the schema.
    pub fn add_operation(&mut self, operation: CompiledOperation) {
        self.operations.push(operation);
    }

    /// Add a fragment definition to the schema.
    pub fn add_fragment(&mut self, fragment: FragmentDefinition) {
        self.fragments.push(fragment);
    }

    /// Add a raw TypeSpec block.
    pub fn add_raw(&mut self, content: impl Into<String>) {
        self.raw_blocks.push(RawTypeSpecBlock::new(content));
    }

    /// Add a raw TypeSpec block with source location.
    pub fn add_raw_with_source(&mut self, content: impl Into<String>, source: SourceSpan) {
        self.raw_blocks
            .push(RawTypeSpecBlock::new(content).with_source(source));
    }

    /// Add an import.
    pub fn add_import(&mut self, import: impl Into<String>) {
        let imp = import.into();
        if !self.imports.contains(&imp) {
            self.imports.push(imp);
        }
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl CompiledModel {
    /// Create a new model.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fields: Vec::new(),
            description: None,
            extensions: Extensions::default(),
            source: None,
        }
    }

    /// Add a field to the model.
    pub fn add_field(&mut self, field: CompiledField) {
        self.fields.push(field);
    }

    /// Set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a decorator.
    pub fn with_decorator(mut self, decorator: impl Into<String>) -> Self {
        self.extensions.add_decorator(decorator);
        self
    }

    /// Set source location.
    pub fn with_source(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }
}

impl CompiledField {
    /// Create a new required field.
    pub fn new(name: impl Into<String>, field_type: CompiledType) -> Self {
        Self {
            name: name.into(),
            field_type,
            optional: None,
            description: None,
            constraints: None,
            extensions: Extensions::default(),
            source: None,
        }
    }

    /// Mark the field as optional.
    pub fn optional(mut self) -> Self {
        self.optional = Some(true);
        self
    }

    /// Add constraints to the field.
    pub fn with_constraints(mut self, constraints: FieldConstraints) -> Self {
        self.constraints = Some(constraints);
        self
    }

    /// Set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a decorator.
    pub fn with_decorator(mut self, decorator: impl Into<String>) -> Self {
        self.extensions.add_decorator(decorator);
        self
    }

    /// Add multiple decorators.
    pub fn with_decorators(mut self, decorators: Vec<impl Into<String>>) -> Self {
        for d in decorators {
            self.extensions.add_decorator(d);
        }
        self
    }

    /// Set source location.
    pub fn with_source(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }

    /// Set format constraint.
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        let constraints = self
            .constraints
            .get_or_insert_with(FieldConstraints::default);
        constraints.format = Some(format.into());
        self
    }
}

impl CompiledType {
    /// Create a string type.
    pub fn string() -> Self {
        Self::Scalar {
            name: ScalarType::String,
        }
    }

    /// Create an int64 type.
    pub fn int64() -> Self {
        Self::Scalar {
            name: ScalarType::Int64,
        }
    }

    /// Create a boolean type.
    pub fn boolean() -> Self {
        Self::Scalar {
            name: ScalarType::Boolean,
        }
    }

    /// Create an array type.
    pub fn array(items: CompiledType) -> Self {
        Self::Array {
            items: Box::new(items),
        }
    }

    /// Create a reference to another type.
    pub fn reference(name: impl Into<String>) -> Self {
        Self::Ref { name: name.into() }
    }

    /// Create a string literal type.
    pub fn literal_string(value: impl Into<String>) -> Self {
        Self::Literal {
            value: LiteralValue::String(value.into()),
        }
    }

    /// Create a scalar type by name (e.g., "utcDateTime", "plainDate").
    pub fn scalar(name: &str) -> Self {
        let scalar_type = match name.to_lowercase().as_str() {
            "string" => ScalarType::String,
            "int32" => ScalarType::Int32,
            "int64" => ScalarType::Int64,
            "float32" => ScalarType::Float32,
            "float64" => ScalarType::Float64,
            "boolean" | "bool" => ScalarType::Boolean,
            "date" => ScalarType::Date,
            "datetime" => ScalarType::Datetime,
            "utcdatetime" => ScalarType::UtcDateTime,
            "offsetdatetime" => ScalarType::OffsetDateTime,
            "plaindate" => ScalarType::PlainDate,
            "plaintime" => ScalarType::PlainTime,
            "duration" => ScalarType::Duration,
            "bytes" => ScalarType::Bytes,
            "url" => ScalarType::Url,
            _ => ScalarType::Unknown,
        };
        Self::Scalar { name: scalar_type }
    }
}

impl CompiledUnionVariant {
    /// Create a variant with a string literal type.
    pub fn literal_string(value: impl Into<String>) -> Self {
        let s = value.into();
        Self {
            name: s.clone(),
            variant_type: Some(CompiledType::Literal {
                value: LiteralValue::String(s),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_serialization() {
        let mut schema = CompiledSchema::new("TestAPI");

        let mut user = CompiledModel::new("User");
        user.add_field(
            CompiledField::new("id", CompiledType::string()).with_constraints(FieldConstraints {
                format: Some("uuid".to_string()),
                ..Default::default()
            }),
        );
        user.add_field(CompiledField::new("email", CompiledType::string()));
        user.add_field(CompiledField::new("name", CompiledType::string()).optional());

        schema.add_model(user);

        let json = schema.to_json().unwrap();
        assert!(json.contains("TestAPI"));
        assert!(json.contains("User"));

        let parsed = CompiledSchema::from_json(&json).unwrap();
        assert_eq!(parsed.name, "TestAPI");
        assert_eq!(parsed.models.len(), 1);
    }

    #[test]
    fn test_operation_serialization_roundtrip() {
        let mut schema = CompiledSchema::new("TestAPI");

        let op = CompiledOperation {
            name: "GetUser".to_string(),
            kind: OperationKind::Query,
            input: None,
            output: CompiledType::reference("User"),
            variables: vec![Variable {
                name: "id".to_string(),
                variable_type: CompiledType::string(),
                required: true,
                default_value: None,
                description: None,
            }],
            selection: Some(SelectionSet {
                fields: vec![
                    FieldSelection {
                        name: "id".to_string(),
                        alias: None,
                        arguments: vec![],
                        selection: None,
                        directives: vec![],
                    },
                    FieldSelection {
                        name: "name".to_string(),
                        alias: Some("userName".to_string()),
                        arguments: vec![],
                        selection: None,
                        directives: vec![],
                    },
                    FieldSelection {
                        name: "posts".to_string(),
                        alias: None,
                        arguments: vec![(
                            "first".to_string(),
                            CompiledType::Scalar {
                                name: ScalarType::Int32,
                            },
                        )],
                        selection: Some(SelectionSet {
                            fields: vec![FieldSelection {
                                name: "title".to_string(),
                                alias: None,
                                arguments: vec![],
                                selection: None,
                                directives: vec![],
                            }],
                            fragment_spreads: vec![],
                            inline_fragments: vec![],
                        }),
                        directives: vec![],
                    },
                ],
                fragment_spreads: vec![FragmentSpread {
                    name: "UserFields".to_string(),
                    directives: vec![],
                }],
                inline_fragments: vec![InlineFragment {
                    type_condition: Some("Admin".to_string()),
                    selection: SelectionSet {
                        fields: vec![FieldSelection {
                            name: "role".to_string(),
                            alias: None,
                            arguments: vec![],
                            selection: None,
                            directives: vec![],
                        }],
                        fragment_spreads: vec![],
                        inline_fragments: vec![],
                    },
                    directives: vec![],
                }],
            }),
            description: Some("Fetch a user by ID".to_string()),
            extensions: Extensions::default(),
            directives: vec![Directive {
                name: "cached".to_string(),
                arguments: vec![("ttl".to_string(), serde_json::json!(60))],
            }],
        };

        schema.add_operation(op);

        let json = schema.to_json().unwrap();
        let parsed = CompiledSchema::from_json(&json).unwrap();

        assert_eq!(parsed.operations.len(), 1);
        let parsed_op = &parsed.operations[0];
        assert_eq!(parsed_op.name, "GetUser");
        assert_eq!(parsed_op.variables.len(), 1);
        assert_eq!(parsed_op.variables[0].name, "id");
        assert!(parsed_op.variables[0].required);
        assert_eq!(parsed_op.directives.len(), 1);
        assert_eq!(parsed_op.directives[0].name, "cached");

        let sel = parsed_op.selection.as_ref().unwrap();
        assert_eq!(sel.fields.len(), 3);
        assert_eq!(sel.fragment_spreads.len(), 1);
        assert_eq!(sel.inline_fragments.len(), 1);
        assert_eq!(sel.fields[1].alias, Some("userName".to_string()));
    }

    #[test]
    fn test_fragment_definition_roundtrip() {
        let mut schema = CompiledSchema::new("TestAPI");

        schema.add_fragment(FragmentDefinition {
            name: "UserFields".to_string(),
            type_condition: "User".to_string(),
            selection: SelectionSet {
                fields: vec![
                    FieldSelection {
                        name: "id".to_string(),
                        alias: None,
                        arguments: vec![],
                        selection: None,
                        directives: vec![],
                    },
                    FieldSelection {
                        name: "email".to_string(),
                        alias: None,
                        arguments: vec![],
                        selection: None,
                        directives: vec![],
                    },
                ],
                fragment_spreads: vec![],
                inline_fragments: vec![],
            },
            directives: vec![],
        });

        let json = schema.to_json().unwrap();
        let parsed = CompiledSchema::from_json(&json).unwrap();
        assert_eq!(parsed.fragments.len(), 1);
        assert_eq!(parsed.fragments[0].name, "UserFields");
        assert_eq!(parsed.fragments[0].type_condition, "User");
        assert_eq!(parsed.fragments[0].selection.fields.len(), 2);
    }
}
