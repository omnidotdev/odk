//! TypeScript Schema Adapter
//!
//! Converts the TypeScript CLI's schema format to the canonical odk-schema format.
//! The TS format uses string-based scalar names and simpler union variants.

use odk_schema::{
    CompiledField, CompiledModel, CompiledSchema, CompiledType, CompiledUnion,
    CompiledUnionVariant, Extensions, LiteralValue, ScalarType,
};
use serde::{Deserialize, Serialize};

/// TypeScript schema format (matches odk-ts compile output).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsCompiledSchema {
    pub name: String,
    pub models: Vec<TsCompiledModel>,
    pub unions: Vec<TsCompiledUnion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsCompiledModel {
    pub name: String,
    pub properties: Vec<TsCompiledProperty>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsCompiledProperty {
    pub name: String,
    #[serde(rename = "type")]
    pub type_info: TsTypeInfo,
    pub optional: bool,
    #[serde(rename = "defaultValue", skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<TsReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsReference {
    pub model: String,
    pub field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsCompiledUnion {
    pub name: String,
    pub variants: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TsTypeInfo {
    Scalar {
        name: String,
        #[serde(rename = "originalType")]
        original_type: String,
    },
    Union {
        name: String,
        variants: Vec<String>,
    },
    Reference {
        name: String,
    },
    Array {
        items: Box<TsTypeInfo>,
    },
}

impl TsCompiledSchema {
    /// Convert to canonical odk-schema format.
    pub fn to_compiled_schema(self) -> CompiledSchema {
        CompiledSchema {
            name: self.name,
            models: self
                .models
                .into_iter()
                .map(|m| m.to_compiled_model())
                .collect(),
            unions: self
                .unions
                .into_iter()
                .map(|u| u.to_compiled_union())
                .collect(),
            operations: Vec::new(),
            fragments: Vec::new(),
            raw_blocks: Vec::new(),
            imports: Vec::new(),
            warnings: None,
        }
    }
}

impl TsCompiledModel {
    fn to_compiled_model(self) -> CompiledModel {
        CompiledModel {
            name: self.name,
            fields: self
                .properties
                .into_iter()
                .map(|p| p.to_compiled_field())
                .collect(),
            description: self.description,
            extensions: Extensions::default(),
            source: None,
        }
    }
}

impl TsCompiledProperty {
    fn to_compiled_field(self) -> CompiledField {
        CompiledField {
            name: self.name,
            field_type: self.type_info.to_compiled_type(),
            optional: if self.optional { Some(true) } else { None },
            description: self.description,
            constraints: None, // TS format doesn't have constraints yet
            extensions: Extensions::default(),
            source: None,
        }
    }
}

impl TsCompiledUnion {
    fn to_compiled_union(self) -> CompiledUnion {
        CompiledUnion {
            name: self.name,
            variants: self
                .variants
                .into_iter()
                .map(|v| CompiledUnionVariant {
                    name: v.clone(),
                    variant_type: Some(CompiledType::Literal {
                        value: LiteralValue::String(v),
                    }),
                })
                .collect(),
            tag: None,
            rename_all: None,
            description: self.description,
            extensions: Extensions::default(),
            source: None,
        }
    }
}

impl TsTypeInfo {
    fn to_compiled_type(self) -> CompiledType {
        match self {
            TsTypeInfo::Scalar { original_type, .. } => CompiledType::Scalar {
                name: parse_scalar_type(&original_type),
            },
            TsTypeInfo::Union { name, .. } => CompiledType::Ref { name },
            TsTypeInfo::Reference { name } => CompiledType::Ref { name },
            TsTypeInfo::Array { items } => CompiledType::Array {
                items: Box::new(items.to_compiled_type()),
                length: None,
            },
        }
    }
}

/// Parse string scalar type to ScalarType enum.
fn parse_scalar_type(s: &str) -> ScalarType {
    match s.to_lowercase().as_str() {
        "string" | "uuid" | "email" | "url" | "uri" => ScalarType::String,
        "int32" | "integer" => ScalarType::Int32,
        "int64" => ScalarType::Int64,
        "float32" | "float" => ScalarType::Float32,
        "float64" | "double" | "number" => ScalarType::Float64,
        "boolean" | "bool" => ScalarType::Boolean,
        "date" => ScalarType::Date,
        "timestamp" | "datetime" => ScalarType::Datetime,
        "bytes" | "binary" => ScalarType::Bytes,
        _ => ScalarType::Unknown,
    }
}
