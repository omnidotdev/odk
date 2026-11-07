//! ODK Plugin Development Kit for Rust
//!
//! This crate provides utilities for building ODK schema plugins in Rust
//! that compile to WebAssembly and can be loaded by the ODK CLI.
//!
//! # Example
//!
//! ```rust,ignore
//! use odk_pdk::prelude::*;
//!
//! // Define your schema
//! fn build_schema() -> CompiledSchema {
//!     let mut schema = CompiledSchema::new("MyAPI");
//!
//!     // Add User model
//!     let mut user = CompiledModel::new("User");
//!     user.add_field(
//!         CompiledField::new("id", CompiledType::string())
//!             .with_constraints(FieldConstraints {
//!                 format: Some("uuid".to_string()),
//!                 ..Default::default()
//!             })
//!     );
//!     user.add_field(CompiledField::new("email", CompiledType::string()));
//!     schema.add_model(user);
//!
//!     schema
//! }
//!
//! // Export the get_schema function
//! odk_export_schema!(build_schema);
//! ```

pub use extism_pdk;
pub use odk_schema;

/// Prelude module - import everything you need with `use odk_pdk::prelude::*`
pub mod prelude {
    pub use crate::odk_schema::{
        CompiledField, CompiledModel, CompiledOperation, CompiledSchema, CompiledType,
        CompiledUnion, CompiledUnionVariant, Directive, FieldConstraints, FieldSelection,
        LiteralValue, OperationKind, ScalarType, SelectionSet,
    };
    // Alias for convenience
    pub use crate::odk_schema::CompiledUnionVariant as UnionVariant;
    pub use extism_pdk::{FnResult, Json, plugin_fn};
}

/// Helper to create a string field with format constraint
pub fn string_field(name: &str) -> odk_schema::CompiledField {
    odk_schema::CompiledField::new(name.to_string(), odk_schema::CompiledType::string())
}

/// Helper to create a UUID field
pub fn uuid_field(name: &str) -> odk_schema::CompiledField {
    odk_schema::CompiledField::new(name.to_string(), odk_schema::CompiledType::string())
        .with_constraints(odk_schema::FieldConstraints {
            format: Some("uuid".to_string()),
            ..Default::default()
        })
}

/// Helper to create an email field
pub fn email_field(name: &str) -> odk_schema::CompiledField {
    odk_schema::CompiledField::new(name.to_string(), odk_schema::CompiledType::string())
        .with_constraints(odk_schema::FieldConstraints {
            format: Some("email".to_string()),
            ..Default::default()
        })
}

/// Helper to create a datetime field
pub fn datetime_field(name: &str) -> odk_schema::CompiledField {
    odk_schema::CompiledField::new(name.to_string(), odk_schema::CompiledType::string())
        .with_constraints(odk_schema::FieldConstraints {
            format: Some("date-time".to_string()),
            ..Default::default()
        })
}

/// Helper to create a string enum union
pub fn string_enum(name: &str, variants: &[&str]) -> odk_schema::CompiledUnion {
    odk_schema::CompiledUnion {
        name: name.to_string(),
        variants: variants
            .iter()
            .map(|v| odk_schema::CompiledUnionVariant {
                name: v.to_string(),
                variant_type: Some(odk_schema::CompiledType::Literal {
                    value: odk_schema::LiteralValue::String(v.to_string()),
                }),
            })
            .collect(),
        ..Default::default()
    }
}

/// Macro to export the get_schema function for your plugin.
///
/// Note: Your plugin crate must have `extism-pdk` as a direct dependency.
///
/// # Example
///
/// ```rust,ignore
/// use odk_pdk::prelude::*;
///
/// fn my_schema() -> CompiledSchema {
///     CompiledSchema::new("MyAPI")
/// }
///
/// odk_pdk::odk_export_schema!(my_schema);
/// ```
#[macro_export]
macro_rules! odk_export_schema {
    ($schema_fn:ident) => {
        #[::extism_pdk::plugin_fn]
        pub fn get_schema(
            _: (),
        ) -> ::extism_pdk::FnResult<::extism_pdk::Json<odk_pdk::odk_schema::CompiledSchema>> {
            Ok(::extism_pdk::Json($schema_fn()))
        }
    };
}
