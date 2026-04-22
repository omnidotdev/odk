//! Source Map Generation
//!
//! Generates .tsp.map sidecar files that map TypeSpec positions
//! back to original source locations (YAML, TS, etc.).

use odk_schema::{CompiledSchema, SourceMap};
use std::path::Path;

/// Generate a source map for TypeSpec output.
///
/// The source map tracks which lines in the generated TypeSpec
/// correspond to which lines in the original source file.
pub fn generate_source_map(
    schema: &CompiledSchema,
    tsp_file: &str,
    source_file: &str,
) -> SourceMap {
    let mut map = SourceMap::new(tsp_file);
    map.add_source(source_file);

    let mut current_line: u32 = 1;

    // Skip imports and namespace (first ~5 lines)
    current_line += 3; // imports
    current_line += schema.imports.len() as u32;
    current_line += 2; // namespace + blank

    // Map unions
    for union in &schema.unions {
        if let Some(ref source) = union.source {
            map.add_mapping(current_line, 0, source.clone());
        }
        // Estimate lines: description + enum + variants + closing
        current_line += 1; // description or empty
        current_line += 1; // enum Name {
        current_line += union.variants.len() as u32;
        current_line += 2; // } + blank
    }

    // Map models
    for model in &schema.models {
        if let Some(ref source) = model.source {
            map.add_mapping(current_line, 0, source.clone());
        }
        // Estimate lines: description + decorators + model + fields + closing
        current_line += 1; // description
        current_line += model.extensions.decorators().len() as u32; // decorators
        current_line += 1; // @jsonSchema
        current_line += 1; // model Name {

        for field in &model.fields {
            if let Some(ref source) = field.source {
                map.add_mapping(current_line, 2, source.clone());
            }
            current_line += 1;
        }

        current_line += 2; // } + blank
    }

    // Map raw blocks
    for raw_block in &schema.raw_blocks {
        if let Some(ref source) = raw_block.source {
            map.add_mapping(current_line, 0, source.clone());
        }
        // Count lines in raw content
        let raw_lines = raw_block.content.lines().count() as u32;
        current_line += raw_lines + 1; // content + blank
    }

    map
}

/// Write a source map to a .tsp.map file.
pub fn write_source_map(map: &SourceMap, tsp_path: &Path) -> std::io::Result<()> {
    let map_path = tsp_path.with_extension("tsp.map");
    let json = map
        .to_json()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    std::fs::write(map_path, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use odk_schema::{CompiledField, CompiledModel, CompiledType, SourceSpan};

    #[test]
    fn test_generate_source_map() {
        let mut schema = CompiledSchema::new("TestAPI");

        let mut user = CompiledModel::new("User").with_source(SourceSpan::new("schema.yaml", 5, 2));
        user.add_field(
            CompiledField::new("id", CompiledType::string()).with_source(SourceSpan::new(
                "schema.yaml",
                7,
                4,
            )),
        );
        schema.add_model(user);

        let map = generate_source_map(&schema, "main.tsp", "schema.yaml");

        assert_eq!(map.sources.len(), 1);
        assert!(map.mappings.len() > 0);
    }
}
