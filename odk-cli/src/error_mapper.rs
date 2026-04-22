//! Error Mapper
//!
//! Maps TypeSpec compiler errors back to original source locations
//! using the .tsp.map sidecar file.

use odk_schema::SourceMap;
use std::path::Path;

/// A remapped error with original source location.
#[derive(Debug)]
pub struct RemappedError {
    /// Original error message
    pub message: String,
    /// Original source file (from source map)
    pub file: Option<String>,
    /// Original line number
    pub line: Option<u32>,
    /// Original column number
    pub column: Option<u32>,
    /// Severity (error, warning)
    pub severity: ErrorSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorSeverity {
    Error,
    Warning,
}

impl RemappedError {
    /// Format as a user-friendly error message.
    pub fn display(&self) -> String {
        match (&self.file, self.line) {
            (Some(file), Some(line)) => {
                let col = self.column.unwrap_or(1);
                format!("{}:{}:{}: {}", file, line, col, self.message)
            }
            _ => self.message.clone(),
        }
    }
}

/// Parse TypeSpec compiler output and extract errors.
pub fn parse_tsp_errors(output: &str) -> Vec<RemappedError> {
    let mut errors = Vec::new();

    for line in output.lines() {
        // TypeSpec errors look like:
        // /path/to/file.tsp:10:5 - error: message
        // or just: error: message
        if let Some(error) = parse_error_line(line) {
            errors.push(error);
        }
    }

    errors
}

fn parse_error_line(line: &str) -> Option<RemappedError> {
    let line = line.trim();

    // Skip empty lines
    if line.is_empty() {
        return None;
    }

    // Check for error/warning keywords
    let severity = if line.contains("error") {
        ErrorSeverity::Error
    } else if line.contains("warning") {
        ErrorSeverity::Warning
    } else {
        return None;
    };

    // Try to parse "file:line:col - error: message" format
    if let Some(dash_pos) = line.find(" - ") {
        let location_part = &line[..dash_pos];
        let message_part = &line[dash_pos + 3..];

        // Parse location (file:line:col)
        let parts: Vec<&str> = location_part.rsplitn(3, ':').collect();
        if parts.len() >= 2 {
            let col = parts[0].parse().ok();
            let line_num = parts[1].parse().ok();
            let file = if parts.len() == 3 {
                Some(parts[2].to_string())
            } else {
                None
            };

            return Some(RemappedError {
                message: message_part.trim().to_string(),
                file,
                line: line_num,
                column: col,
                severity,
            });
        }
    }

    // Fallback: just extract the message
    Some(RemappedError {
        message: line.to_string(),
        file: None,
        line: None,
        column: None,
        severity,
    })
}

/// Remap errors using a source map.
pub fn remap_errors(errors: Vec<RemappedError>, source_map: &SourceMap) -> Vec<RemappedError> {
    errors
        .into_iter()
        .map(|mut error| {
            if let Some(line) = error.line
                && let Some(original) = source_map.lookup(line, error.column.unwrap_or(0))
            {
                error.file = Some(original.file.clone());
                error.line = Some(original.line);
                error.column = Some(original.column);
            }
            error
        })
        .collect()
}

/// Load a source map from a .tsp.map file.
pub fn load_source_map(tsp_path: &Path) -> Option<SourceMap> {
    let map_path = tsp_path.with_extension("tsp.map");
    if !map_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&map_path).ok()?;
    SourceMap::from_json(&content).ok()
}

/// Process TypeSpec compiler output and remap errors to original sources.
pub fn process_tsp_output(output: &str, tsp_path: &Path) -> Vec<RemappedError> {
    let errors = parse_tsp_errors(output);

    if let Some(source_map) = load_source_map(tsp_path) {
        remap_errors(errors, &source_map)
    } else {
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_line() {
        let line = "/tmp/main.tsp:10:5 - error: Unknown identifier 'foo'";
        let error = parse_error_line(line).unwrap();

        assert_eq!(error.line, Some(10));
        assert_eq!(error.column, Some(5));
        assert!(error.message.contains("Unknown identifier"));
    }

    #[test]
    fn test_parse_warning() {
        let line = "warning: Unused import";
        let error = parse_error_line(line).unwrap();

        assert_eq!(error.severity, ErrorSeverity::Warning);
    }
}
