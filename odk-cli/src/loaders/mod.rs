//! Schema loaders for various input formats.
//!
//! ODK supports multiple input formats:
//! - **YAML/JSON**: Declarative schema definition
//! - **TypeScript**: Programmatic with Zod/ArkType/Valibot
//! - **WASM**: Plugins in any Extism-supported language

pub mod yaml;

pub use yaml::load_yaml_schema;
