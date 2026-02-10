//! Source mapping types for ODK Schema
//!
//! These types enable tracing generated TypeSpec back to the original
//! source files (YAML, TypeScript, etc.) for better error messages.

use serde::{Deserialize, Serialize};

/// A span in a source file.
///
/// Used to track where schema elements originated from,
/// enabling error messages that point to the original source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Path to the source file (relative or absolute)
    pub file: String,
    /// 1-based line number
    pub line: u32,
    /// 1-based column number
    pub column: u32,
    /// Optional end position for ranges
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
}

impl SourceSpan {
    /// Create a new source span.
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column,
            end_line: None,
            end_column: None,
        }
    }

    /// Create a source span with an end position.
    pub fn with_end(mut self, end_line: u32, end_column: u32) -> Self {
        self.end_line = Some(end_line);
        self.end_column = Some(end_column);
        self
    }

    /// Format as "file:line:column" for error messages.
    pub fn display(&self) -> String {
        format!("{}:{}:{}", self.file, self.line, self.column)
    }
}

/// A raw TypeSpec block to be passed through verbatim.
///
/// Used for escape hatches where users want to write TypeSpec directly
/// without going through the ODK IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTypeSpecBlock {
    /// The raw TypeSpec source code
    pub content: String,
    /// Optional source location for error mapping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceSpan>,
    /// Optional description/comment about this block
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl RawTypeSpecBlock {
    /// Create a new raw block.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            source: None,
            description: None,
        }
    }

    /// Add source location.
    pub fn with_source(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }

    /// Add description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// A source map entry mapping generated TypeSpec to original source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMapEntry {
    /// Position in the generated TypeSpec file
    pub generated: GeneratedPosition,
    /// Position in the original source file
    pub original: SourceSpan,
}

/// Position in a generated file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedPosition {
    /// 1-based line number
    pub line: u32,
    /// 1-based column number
    pub column: u32,
}

/// A complete source map for a generated TypeSpec file.
///
/// Format inspired by JavaScript source maps but simplified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMap {
    /// Version of the source map format
    pub version: u32,
    /// The generated TypeSpec file
    pub file: String,
    /// List of original source files
    pub sources: Vec<String>,
    /// Mapping entries
    pub mappings: Vec<SourceMapEntry>,
}

impl SourceMap {
    /// Create a new source map.
    pub fn new(file: impl Into<String>) -> Self {
        Self {
            version: 1,
            file: file.into(),
            sources: Vec::new(),
            mappings: Vec::new(),
        }
    }

    /// Add a source file.
    pub fn add_source(&mut self, source: impl Into<String>) {
        let s = source.into();
        if !self.sources.contains(&s) {
            self.sources.push(s);
        }
    }

    /// Add a mapping entry.
    pub fn add_mapping(
        &mut self,
        generated_line: u32,
        generated_column: u32,
        original: SourceSpan,
    ) {
        self.add_source(&original.file);
        self.mappings.push(SourceMapEntry {
            generated: GeneratedPosition {
                line: generated_line,
                column: generated_column,
            },
            original,
        });
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Look up the original source location for a generated position.
    pub fn lookup(&self, line: u32, column: u32) -> Option<&SourceSpan> {
        // Find the closest mapping at or before the given position
        self.mappings
            .iter()
            .filter(|m| {
                m.generated.line < line
                    || (m.generated.line == line && m.generated.column <= column)
            })
            .max_by_key(|m| (m.generated.line, m.generated.column))
            .map(|m| &m.original)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_span() {
        let span = SourceSpan::new("schema.yaml", 10, 5);
        assert_eq!(span.display(), "schema.yaml:10:5");
    }

    #[test]
    fn test_raw_block() {
        let block = RawTypeSpecBlock::new("@route(\"/api\")\nop getData(): void;")
            .with_source(SourceSpan::new("schema.yaml", 15, 1))
            .with_description("Custom API endpoint");

        let json = serde_json::to_string_pretty(&block).unwrap();
        assert!(json.contains("@route"));
        assert!(json.contains("schema.yaml"));
    }

    #[test]
    fn test_source_map() {
        let mut map = SourceMap::new("main.tsp");
        map.add_mapping(5, 0, SourceSpan::new("schema.yaml", 3, 2));
        map.add_mapping(10, 0, SourceSpan::new("schema.yaml", 8, 4));

        // Test lookup
        let found = map.lookup(7, 0);
        assert!(found.is_some());
        assert_eq!(found.unwrap().line, 3);

        let json = map.to_json().unwrap();
        assert!(json.contains("schema.yaml"));
    }
}
