//! TypeSpec Bridge
//!
//! Converts CompiledSchema to TypeSpec source code.
//!
//! Features:
//! - Emits models, unions, and fields with constraints
//! - Supports decorator passthrough via extensions
//! - Emits raw TypeSpec blocks verbatim
//! - Handles custom imports

use odk_schema::{
    CompiledField, CompiledModel, CompiledSchema, CompiledType, CompiledUnion, FieldConstraints,
    LiteralValue, ScalarType,
};

/// Convert a CompiledSchema to TypeSpec source code.
pub fn compiled_to_typespec(schema: &CompiledSchema, namespace: &str) -> String {
    let mut lines = Vec::new();

    // Emit imports from schema (must come first)
    for import in &schema.imports {
        lines.push(format!("import \"{}\";", import));
    }

    if !schema.imports.is_empty() {
        lines.push(String::new());
    }

    // Emit 'using' statements to bring decorators into scope
    for import in &schema.imports {
        if let Some(using_ns) = import_to_using_namespace(import) {
            lines.push(format!("using {};", using_ns));
        }
    }

    if !schema.imports.is_empty() {
        lines.push(String::new());
    }

    // Namespace
    lines.push(format!("namespace {};", sanitize_identifier(namespace)));
    lines.push(String::new());

    // Emit unions first (they may be referenced by models)
    for union in &schema.unions {
        lines.push(emit_union(union));
        lines.push(String::new());
    }

    // Emit models
    for model in &schema.models {
        lines.push(emit_model(model));
        lines.push(String::new());
    }

    // Emit operations
    for op in &schema.operations {
        lines.push(emit_operation(op));
        lines.push(String::new());
    }

    // Emit raw TypeSpec blocks
    for raw_block in &schema.raw_blocks {
        if let Some(desc) = &raw_block.description {
            lines.push(format!("// {}", desc));
        }
        lines.push(raw_block.content.clone());
        lines.push(String::new());
    }

    lines.join("\n")
}

fn emit_union(union: &CompiledUnion) -> String {
    let mut lines = Vec::new();

    if let Some(desc) = &union.description {
        lines.push(format!("/** {} */", desc));
    }

    // Check if it's a string literal enum
    let is_string_enum = union.variants.iter().all(|v| {
        matches!(
            &v.variant_type,
            Some(CompiledType::Literal {
                value: LiteralValue::String(_)
            })
        )
    });

    if is_string_enum {
        lines.push(format!("enum {} {{", sanitize_identifier(&union.name)));
        for variant in &union.variants {
            if let Some(CompiledType::Literal {
                value: LiteralValue::String(s),
            }) = &variant.variant_type
            {
                if is_valid_identifier(&variant.name) && variant.name == *s {
                    lines.push(format!("  {},", variant.name));
                } else {
                    lines.push(format!(
                        "  {}: \"{}\",",
                        sanitize_identifier(&variant.name),
                        s
                    ));
                }
            }
        }
        lines.push("}".to_string());
    } else {
        // Union type alias
        let variant_types: Vec<String> = union
            .variants
            .iter()
            .map(|v| {
                v.variant_type
                    .as_ref()
                    .map(type_to_string)
                    .unwrap_or_else(|| v.name.clone())
            })
            .collect();
        lines.push(format!(
            "alias {} = {};",
            sanitize_identifier(&union.name),
            variant_types.join(" | ")
        ));
    }

    lines.join("\n")
}

fn emit_model(model: &CompiledModel) -> String {
    let mut lines = Vec::new();

    if let Some(desc) = &model.description {
        lines.push(format!("/** {} */", desc));
    }

    // Emit decorators from extensions
    for decorator in model.extensions.decorators() {
        lines.push(decorator.to_string());
    }

    lines.push(format!("model {} {{", sanitize_identifier(&model.name)));

    for field in &model.fields {
        lines.push(emit_field(field));
    }

    lines.push("}".to_string());

    lines.join("\n")
}

fn emit_field(field: &CompiledField) -> String {
    let mut decorators = Vec::new();

    // Add decorators from extensions first
    for decorator in field.extensions.decorators() {
        decorators.push(decorator.to_string());
    }

    // Add constraint decorators
    if let Some(c) = &field.constraints {
        decorators.extend(constraints_to_decorators(c));
    }

    let decorator_str = if decorators.is_empty() {
        String::new()
    } else {
        format!("{} ", decorators.join(" "))
    };

    let optional_mark = if field.optional.unwrap_or(false) {
        "?"
    } else {
        ""
    };

    let type_str = type_to_string(&field.field_type);

    format!(
        "  {}{}{}: {};",
        decorator_str,
        sanitize_identifier(&field.name),
        optional_mark,
        type_str
    )
}

fn constraints_to_decorators(c: &FieldConstraints) -> Vec<String> {
    let mut decorators = Vec::new();

    if let Some(v) = c.min_length {
        decorators.push(format!("@minLength({})", v));
    }
    if let Some(v) = c.max_length {
        decorators.push(format!("@maxLength({})", v));
    }
    if let Some(v) = c.min_value {
        decorators.push(format!("@minValue({})", v));
    }
    if let Some(v) = c.max_value {
        decorators.push(format!("@maxValue({})", v));
    }
    if let Some(v) = &c.pattern {
        decorators.push(format!("@pattern(\"{}\")", escape_string(v)));
    }
    if let Some(v) = c.multiple_of {
        decorators.push(format!("@multipleOf({})", v));
    }
    if let Some(v) = &c.format {
        decorators.push(format!("@format(\"{}\")", v));
    }

    decorators
}

fn emit_operation(op: &odk_schema::CompiledOperation) -> String {
    let mut lines = Vec::new();

    if let Some(desc) = &op.description {
        lines.push(format!("/** {} */", desc));
    }

    // Emit directives
    for directive in &op.directives {
        let args_str = if directive.arguments.is_empty() {
            String::new()
        } else {
            let args: Vec<String> = directive
                .arguments
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect();
            format!("({})", args.join(", "))
        };
        lines.push(format!("@{}{}", directive.name, args_str));
    }

    // Build parameter list from variables
    let params = if op.variables.is_empty() {
        String::new()
    } else {
        let param_strs: Vec<String> = op
            .variables
            .iter()
            .map(|v| {
                let opt = if v.required { "" } else { "?" };
                format!(
                    "{}{}: {}",
                    sanitize_identifier(&v.name),
                    opt,
                    type_to_string(&v.variable_type)
                )
            })
            .collect();
        param_strs.join(", ")
    };

    let return_type = type_to_string(&op.output);

    lines.push(format!(
        "op {}({}): {};",
        sanitize_identifier(&op.name),
        params,
        return_type
    ));

    lines.join("\n")
}

fn type_to_string(t: &CompiledType) -> String {
    match t {
        CompiledType::Scalar { name } => scalar_to_typespec(name),
        CompiledType::Array { items, .. } => format!("{}[]", type_to_string(items)),
        CompiledType::Ref { name } => sanitize_identifier(name),
        CompiledType::Literal { value } => match value {
            LiteralValue::String(s) => format!("\"{}\"", escape_string(s)),
            LiteralValue::Number(n) => n.to_string(),
            LiteralValue::Bool(b) => b.to_string(),
        },
        CompiledType::Union { variants } => variants
            .iter()
            .map(type_to_string)
            .collect::<Vec<_>>()
            .join(" | "),
        CompiledType::Object { fields } => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|f| {
                    let opt = if f.optional.unwrap_or(false) { "?" } else { "" };
                    format!("{}{}: {}", f.name, opt, type_to_string(&f.field_type))
                })
                .collect();
            format!("{{ {} }}", field_strs.join("; "))
        }
    }
}

fn scalar_to_typespec(scalar: &ScalarType) -> String {
    match scalar {
        ScalarType::String => "string".to_string(),
        ScalarType::Int32 => "int32".to_string(),
        ScalarType::Int64 => "int64".to_string(),
        ScalarType::Float32 => "float32".to_string(),
        ScalarType::Float64 => "float64".to_string(),
        ScalarType::Boolean => "boolean".to_string(),
        ScalarType::Date => "plainDate".to_string(),
        ScalarType::Datetime => "utcDateTime".to_string(),
        ScalarType::UtcDateTime => "utcDateTime".to_string(),
        ScalarType::OffsetDateTime => "offsetDateTime".to_string(),
        ScalarType::PlainDate => "plainDate".to_string(),
        ScalarType::PlainTime => "plainTime".to_string(),
        ScalarType::Duration => "duration".to_string(),
        ScalarType::Bytes => "bytes".to_string(),
        ScalarType::Url => "url".to_string(),
        ScalarType::Unknown => "unknown".to_string(),
    }
}

fn sanitize_identifier(name: &str) -> String {
    let mut sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        sanitized = "_".to_string();
    }

    if sanitized.chars().next().unwrap().is_numeric() {
        sanitized = format!("_{}", sanitized);
    }

    sanitized
}

fn is_valid_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let first = name.chars().next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }

    name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Map a TypeSpec import to its corresponding 'using' namespace.
/// Returns None if the package doesn't expose decorators that need 'using'.
fn import_to_using_namespace(import: &str) -> Option<&'static str> {
    match import {
        "@typespec/http" => Some("TypeSpec.Http"),
        "@typespec/rest" => Some("TypeSpec.Rest"),
        "@typespec/openapi" => Some("TypeSpec.OpenAPI"),
        "@typespec/openapi3" => Some("TypeSpec.OpenAPI"),
        "@typespec/json-schema" => Some("TypeSpec.JsonSchema"),
        "@typespec/protobuf" => Some("TypeSpec.Protobuf"),
        "@typespec/versioning" => Some("TypeSpec.Versioning"),
        "@typespec/xml" => Some("TypeSpec.Xml"),
        "@typespec/events" => Some("TypeSpec.Events"),
        "@typespec/streams" => Some("TypeSpec.Streams"),
        "@typespec/sse" => Some("TypeSpec.SSE"),
        // Emitter-only packages (no decorators to bring into scope)
        // "@omnidotdev/typespec-drizzle" - emitter only, no namespace
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_schema() {
        let mut schema = CompiledSchema::new("TestAPI");

        let mut user = CompiledModel::new("User");
        user.add_field(CompiledField::new("id", CompiledType::string()));
        user.add_field(CompiledField::new("name", CompiledType::string()).optional());

        schema.add_model(user);

        let tsp = compiled_to_typespec(&schema, "TestAPI");

        assert!(tsp.contains("namespace TestAPI;"));
        assert!(tsp.contains("model User {"));
        assert!(tsp.contains("id: string;"));
        assert!(tsp.contains("name?: string;"));
        // Should not contain auto-imports
        assert!(!tsp.contains("@jsonSchema"));
        assert!(!tsp.contains("using TypeSpec"));
    }

    #[test]
    fn test_schema_with_imports() {
        let mut schema = CompiledSchema::new("TestAPI");
        schema.add_import("@typespec/http");
        schema.add_import("@omnidotdev/typespec-drizzle");

        let tsp = compiled_to_typespec(&schema, "TestAPI");

        assert!(tsp.contains("import \"@typespec/http\";"));
        assert!(tsp.contains("import \"@omnidotdev/typespec-drizzle\";"));
    }
}
