//! Schema loaders for various input formats.
//!
//! ODK supports multiple input formats:
//! - **YAML/JSON**: Declarative schema definition
//! - **GraphQL**: SDL schemas and operation documents
//! - **TypeScript**: Programmatic with Zod/ArkType/Valibot
//! - **WASM**: Plugins in any Extism-supported language

pub mod graphql;
pub mod introspect;
pub mod yaml;

pub use graphql::{load_graphql_operations, load_graphql_schema};
pub use introspect::introspect_schema;
pub use yaml::load_yaml_schema;
