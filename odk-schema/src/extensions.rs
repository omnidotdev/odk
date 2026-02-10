//! Extension types for ODK Schema
//!
//! Extensions allow passing through features that don't map directly to TypeSpec,
//! such as Zod transforms, custom refinements, and arbitrary decorators.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Known extension types that ODK understands.
///
/// These have defined semantics and can be processed specially by emitters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum KnownExtension {
    /// Zod `.transform()` - runtime transformation function.
    /// Stored as serialized code string since it can't be executed in Rust.
    ZodTransform {
        /// The transform function code as a string
        code: String,
    },

    /// Zod `.refine()` - custom validation that doesn't map to JSON Schema.
    ZodRefinement {
        /// The refinement function code as a string
        code: String,
        /// Optional error message
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    /// Arbitrary TypeSpec decorator to pass through verbatim.
    /// Example: "@doc('Primary key')", "@id", "@manyToOne(User)"
    Decorator {
        /// The full decorator string including @
        value: String,
    },

    /// OpenAPI extension (x-* properties).
    OpenApi {
        /// Extension key (e.g., "x-custom-field")
        key: String,
        /// Extension value (any JSON value)
        value: serde_json::Value,
    },

    /// Drizzle-specific decorator.
    /// Example: "@table", "@primaryKey"
    Drizzle {
        /// The decorator name
        decorator: String,
        /// Optional arguments
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<Vec<String>>,
    },
}

/// Container for both known and unknown extensions.
///
/// Known extensions are validated and have defined semantics.
/// Unknown extensions are passed through as opaque JSON for forward compatibility.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Extensions {
    /// Known extensions that ODK understands
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known: Vec<KnownExtension>,

    /// Unknown extensions passed through as opaque JSON.
    /// This allows new extension types to be added without breaking existing code.
    #[serde(default, skip_serializing_if = "HashMap::is_empty", flatten)]
    pub unknown: HashMap<String, serde_json::Value>,
}

impl Extensions {
    /// Create empty extensions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are no extensions.
    pub fn is_empty(&self) -> bool {
        self.known.is_empty() && self.unknown.is_empty()
    }

    /// Add a known extension.
    pub fn add_known(&mut self, ext: KnownExtension) {
        self.known.push(ext);
    }

    /// Add an unknown extension.
    pub fn add_unknown(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.unknown.insert(key.into(), value);
    }

    /// Add a decorator extension.
    pub fn add_decorator(&mut self, decorator: impl Into<String>) {
        self.known.push(KnownExtension::Decorator {
            value: decorator.into(),
        });
    }

    /// Get all decorator extensions.
    pub fn decorators(&self) -> Vec<&str> {
        self.known
            .iter()
            .filter_map(|ext| match ext {
                KnownExtension::Decorator { value } => Some(value.as_str()),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extensions_serialization() {
        let mut ext = Extensions::new();
        ext.add_decorator("@id");
        ext.add_decorator("@doc('Primary key')");
        ext.add_known(KnownExtension::ZodTransform {
            code: "x => x.toUpperCase()".into(),
        });

        let json = serde_json::to_string_pretty(&ext).unwrap();
        assert!(json.contains("@id"));
        assert!(json.contains("zodTransform"));

        let parsed: Extensions = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.known.len(), 3);
    }

    #[test]
    fn test_decorators_helper() {
        let mut ext = Extensions::new();
        ext.add_decorator("@table");
        ext.add_decorator("@primaryKey");

        let decorators = ext.decorators();
        assert_eq!(decorators, vec!["@table", "@primaryKey"]);
    }
}
