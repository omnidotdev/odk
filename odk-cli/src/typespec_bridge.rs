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

/// Protobuf-specific emission options.
///
/// When present, the bridge decorates the output with the `@typespec/protobuf`
/// decorators the emitter requires (`@package` on the namespace, `@message` on
/// each model, `@field(n)` on each property), so `odk compile -e
/// @typespec/protobuf` produces a `.proto` without the author hand-writing
/// protobuf decorators
#[derive(Debug, Clone)]
pub struct ProtobufOptions {
    /// The protobuf package name (also drives the emitted `.proto` file name)
    pub package: String,
}

/// Target-specific options that influence TypeSpec emission.
#[derive(Debug, Clone, Default)]
pub struct EmitOptions {
    /// Set when `@typespec/protobuf` is a requested emitter
    pub protobuf: Option<ProtobufOptions>,
    /// Set when `@typespec/json-schema` is a requested emitter. The user
    /// namespace is then marked `@jsonSchema` so emission is scoped to it and
    /// an imported library's types (e.g. Drizzle option models) do not leak
    pub json_schema: bool,
}

/// Derive emission options from the requested emitters and namespace.
///
/// When `@typespec/protobuf` is among the emitters, the protobuf package name
/// is derived from the namespace (sanitized, lowercased) so compilation is
/// turnkey without the author supplying protobuf decorators.
pub fn emit_options_for(emitters: &[String], namespace: &str) -> EmitOptions {
    let protobuf = emitters
        .iter()
        .any(|e| e == "@typespec/protobuf")
        .then(|| ProtobufOptions {
            package: sanitize_identifier(namespace).to_lowercase(),
        });
    let json_schema = emitters.iter().any(|e| e == "@typespec/json-schema");
    EmitOptions {
        protobuf,
        json_schema,
    }
}

/// Convert a CompiledSchema to TypeSpec source code.
pub fn compiled_to_typespec(schema: &CompiledSchema, namespace: &str) -> String {
    compiled_to_typespec_with_options(schema, namespace, &EmitOptions::default())
}

/// Reject a schema the TypeSpec `compile` path cannot yet emit correctly.
///
/// The bridge emits string enums, but a data-carrying (tagged) union would come
/// out as an undiscriminated union alias, silently dropping the discriminator
/// from the OpenAPI / JSON Schema output. Until the discriminator path exists,
/// return a clear error so `odk compile` fails loudly instead. Returns `None`
/// when every union is a plain string enum.
pub fn unsupported_feature(schema: &CompiledSchema) -> Option<String> {
    schema
        .unions
        .iter()
        .find(|u| u.is_data_carrying())
        .map(|u| {
            format!(
                "`odk compile` does not yet support the data-carrying (tagged) union `{}`; the \
                 TypeSpec discriminator path is not implemented, so the discriminator would be \
                 dropped from the OpenAPI / JSON Schema output. Use `odk generate -t rust,typescript` \
                 for tagged unions.",
                u.name
            )
        })
}

/// Convert a CompiledSchema to TypeSpec source code with target options.
pub fn compiled_to_typespec_with_options(
    schema: &CompiledSchema,
    namespace: &str,
    options: &EmitOptions,
) -> String {
    let mut lines = Vec::new();
    let protobuf = options.protobuf.is_some();

    // The effective import list. When protobuf is targeted, the library must be
    // imported so its decorators resolve, even if the schema did not declare it
    let mut imports: Vec<String> = schema.imports.clone();
    if protobuf && !imports.iter().any(|i| i == "@typespec/protobuf") {
        imports.push("@typespec/protobuf".to_string());
    }
    if options.json_schema && !imports.iter().any(|i| i == "@typespec/json-schema") {
        imports.push("@typespec/json-schema".to_string());
    }

    // Emit imports (must come first)
    for import in &imports {
        lines.push(format!("import \"{}\";", import));
    }

    if !imports.is_empty() {
        lines.push(String::new());
    }

    // Emit 'using' statements to bring decorators into scope
    for import in &imports {
        if let Some(using_ns) = import_to_using_namespace(import) {
            lines.push(format!("using {};", using_ns));
        }
    }

    if !imports.is_empty() {
        lines.push(String::new());
    }

    // Namespace, optionally declared as a protobuf package. The object-literal
    // argument syntax (`@package({ name: ... })`) is what @typespec/protobuf
    // accepts; the value-literal form (`#{ ... }`) is rejected
    if let Some(pb) = &options.protobuf {
        lines.push(format!(
            "@package({{ name: \"{}\" }})",
            escape_string(&pb.package)
        ));
    }
    // Scope JSON Schema emission to this namespace (paired with
    // emitAllModels: false), so imported libraries' types do not leak.
    // Fully qualified to avoid needing `using TypeSpec.JsonSchema`, whose
    // decorators (e.g. `id`) would collide with an imported library's
    if options.json_schema {
        lines.push("@TypeSpec.JsonSchema.jsonSchema".to_string());
    }
    lines.push(format!("namespace {};", sanitize_identifier(namespace)));
    lines.push(String::new());

    // Emit unions first (they may be referenced by models)
    for union in &schema.unions {
        lines.push(emit_union(union, protobuf));
        lines.push(String::new());
    }

    // Emit models
    for model in &schema.models {
        lines.push(emit_model(model, protobuf));
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

fn emit_union(union: &CompiledUnion, protobuf: bool) -> String {
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
        for (index, variant) in union.variants.iter().enumerate() {
            if protobuf {
                // Protobuf enums require explicit integer members, first == 0
                lines.push(format!(
                    "  {}: {},",
                    sanitize_identifier(&variant.name),
                    index
                ));
            } else if let Some(CompiledType::Literal {
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
                    .map(|t| type_to_string(t, false))
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

fn emit_model(model: &CompiledModel, protobuf: bool) -> String {
    let mut lines = Vec::new();

    if let Some(desc) = &model.description {
        lines.push(format!("/** {} */", desc));
    }

    // Emit decorators from extensions
    for decorator in model.extensions.decorators() {
        lines.push(decorator.to_string());
    }

    // Protobuf requires each message model to be marked
    if protobuf {
        lines.push("@message".to_string());
    }

    lines.push(format!("model {} {{", sanitize_identifier(&model.name)));

    // Protobuf field indices are 1-based and follow declaration order
    for (idx, field) in model.fields.iter().enumerate() {
        lines.push(emit_field(field, idx + 1, protobuf));
    }

    lines.push("}".to_string());

    lines.join("\n")
}

fn emit_field(field: &CompiledField, index: usize, protobuf: bool) -> String {
    let mut decorators = Vec::new();

    // Protobuf field index must come first so the property is a numbered field
    if protobuf {
        decorators.push(format!("@field({})", index));
    }

    // Add decorators from extensions
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

    let type_str = type_to_string(&field.field_type, protobuf);

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
                    type_to_string(&v.variable_type, false)
                )
            })
            .collect();
        param_strs.join(", ")
    };

    let return_type = type_to_string(&op.output, false);

    lines.push(format!(
        "op {}({}): {};",
        sanitize_identifier(&op.name),
        params,
        return_type
    ));

    lines.join("\n")
}

fn type_to_string(t: &CompiledType, protobuf: bool) -> String {
    match t {
        CompiledType::Scalar { name } => scalar_to_typespec(name, protobuf),
        CompiledType::Array { items, .. } => format!("{}[]", type_to_string(items, protobuf)),
        CompiledType::Ref { name } => sanitize_identifier(name),
        CompiledType::Literal { value } => match value {
            LiteralValue::String(s) => format!("\"{}\"", escape_string(s)),
            LiteralValue::Number(n) => n.to_string(),
            LiteralValue::Bool(b) => b.to_string(),
        },
        CompiledType::Union { variants } => variants
            .iter()
            .map(|v| type_to_string(v, protobuf))
            .collect::<Vec<_>>()
            .join(" | "),
        CompiledType::Object { fields } => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|f| {
                    let opt = if f.optional.unwrap_or(false) { "?" } else { "" };
                    format!(
                        "{}{}: {}",
                        f.name,
                        opt,
                        type_to_string(&f.field_type, protobuf)
                    )
                })
                .collect();
            format!("{{ {} }}", field_strs.join("; "))
        }
    }
}

fn scalar_to_typespec(scalar: &ScalarType, protobuf: bool) -> String {
    // Protobuf has no native temporal, URL, or unknown scalar, so those degrade
    // to string (RFC 3339 / plain text) to keep protobuf emission turnkey.
    // bytes and the numeric/bool/string scalars map natively
    if protobuf {
        match scalar {
            ScalarType::Date
            | ScalarType::Datetime
            | ScalarType::UtcDateTime
            | ScalarType::OffsetDateTime
            | ScalarType::PlainDate
            | ScalarType::PlainTime
            | ScalarType::Duration
            | ScalarType::Url
            | ScalarType::Unknown => return "string".to_string(),
            _ => {}
        }
    }

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
        // @typespec/json-schema is intentionally absent: the bridge references
        // its one decorator fully-qualified (@TypeSpec.JsonSchema.jsonSchema) on
        // the namespace, so bringing the whole namespace into scope with `using`
        // would only risk collisions (e.g. its `id` vs Drizzle's `id`)
        "@typespec/protobuf" => Some("TypeSpec.Protobuf"),
        "@typespec/versioning" => Some("TypeSpec.Versioning"),
        "@typespec/xml" => Some("TypeSpec.Xml"),
        "@typespec/events" => Some("TypeSpec.Events"),
        "@typespec/streams" => Some("TypeSpec.Streams"),
        "@typespec/sse" => Some("TypeSpec.SSE"),
        // Drizzle ships its decorators (@table/@id/@manyToOne/...) under the
        // `Drizzle` namespace, so they must be brought into scope for bare
        // decorator usage to resolve (matches the library's own examples)
        "@omnidotdev/typespec-drizzle" => Some("Drizzle"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_allows_a_string_enum() {
        let mut schema = CompiledSchema::new("S");
        schema.add_union(string_enum("Status", &["draft", "published"]));
        assert!(unsupported_feature(&schema).is_none());
    }

    #[test]
    fn compile_rejects_a_data_carrying_union() {
        let mut schema = CompiledSchema::new("S");
        schema.add_union(CompiledUnion {
            name: "OpNode".to_string(),
            tag: Some("op".to_string()),
            variants: vec![odk_schema::CompiledUnionVariant {
                name: "cube".to_string(),
                variant_type: Some(CompiledType::Object { fields: vec![] }),
            }],
            ..Default::default()
        });
        let err = unsupported_feature(&schema).expect("tagged union should be rejected");
        assert!(err.contains("OpNode") && err.contains("compile"), "{err}");
    }

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
        // Imports whose decorators live under a namespace must also be brought
        // into scope, or bare decorators (@table/@id/...) fail to resolve
        assert!(tsp.contains("using TypeSpec.Http;"));
        assert!(tsp.contains("using Drizzle;"));
    }

    #[test]
    fn json_schema_target_scopes_emission_to_the_namespace() {
        let mut schema = CompiledSchema::new("TestAPI");
        schema.add_model(CompiledModel::new("User"));
        let opts = EmitOptions {
            json_schema: true,
            ..Default::default()
        };

        let tsp = compiled_to_typespec_with_options(&schema, "TestAPI", &opts);

        assert!(tsp.contains("import \"@typespec/json-schema\";"));
        // the namespace is marked for emission with the fully-qualified
        // decorator (paired with emitAllModels: false) so only the user's
        // types emit and an imported library's models do not leak
        assert!(tsp.contains("@TypeSpec.JsonSchema.jsonSchema"));
        // never via `using`, which would collide with a library's decorators
        assert!(!tsp.contains("using TypeSpec.JsonSchema;"));
    }

    fn protobuf_schema() -> CompiledSchema {
        let mut schema = CompiledSchema::new("Blog");
        let mut post = CompiledModel::new("Post");
        post.add_field(CompiledField::new("id", CompiledType::int64()));
        post.add_field(CompiledField::new("title", CompiledType::string()));
        schema.add_model(post);
        schema
    }

    #[test]
    fn test_protobuf_injects_package_on_namespace() {
        let schema = protobuf_schema();
        let opts = EmitOptions {
            protobuf: Some(ProtobufOptions {
                package: "blog".to_string(),
            }),
            ..Default::default()
        };

        let tsp = compiled_to_typespec_with_options(&schema, "Blog", &opts);

        // The @package decorator must precede the namespace, with the plain
        // object-literal arg syntax that @typespec/protobuf 0.76 accepts
        assert!(
            tsp.contains("@package({ name: \"blog\" })\nnamespace Blog;"),
            "expected @package decorator before namespace, got:\n{tsp}"
        );
    }

    #[test]
    fn test_protobuf_auto_imports_and_using() {
        let schema = protobuf_schema();
        let opts = EmitOptions {
            protobuf: Some(ProtobufOptions {
                package: "blog".to_string(),
            }),
            ..Default::default()
        };

        let tsp = compiled_to_typespec_with_options(&schema, "Blog", &opts);

        assert!(tsp.contains("import \"@typespec/protobuf\";"));
        assert!(tsp.contains("using TypeSpec.Protobuf;"));
    }

    #[test]
    fn test_protobuf_decorates_models_and_fields() {
        let schema = protobuf_schema();
        let opts = EmitOptions {
            protobuf: Some(ProtobufOptions {
                package: "blog".to_string(),
            }),
            ..Default::default()
        };

        let tsp = compiled_to_typespec_with_options(&schema, "Blog", &opts);

        assert!(tsp.contains("@message\nmodel Post {"), "got:\n{tsp}");
        // Field indices are 1-based and follow declaration order
        assert!(tsp.contains("@field(1) id: int64;"), "got:\n{tsp}");
        assert!(tsp.contains("@field(2) title: string;"), "got:\n{tsp}");
    }

    #[test]
    fn test_protobuf_does_not_duplicate_existing_import() {
        let mut schema = protobuf_schema();
        schema.add_import("@typespec/protobuf");
        let opts = EmitOptions {
            protobuf: Some(ProtobufOptions {
                package: "blog".to_string(),
            }),
            ..Default::default()
        };

        let tsp = compiled_to_typespec_with_options(&schema, "Blog", &opts);

        assert_eq!(
            tsp.matches("import \"@typespec/protobuf\";").count(),
            1,
            "protobuf import must not be duplicated, got:\n{tsp}"
        );
    }

    #[test]
    fn test_emit_options_none_without_protobuf_target() {
        let emitters = vec![
            "@typespec/openapi3".to_string(),
            "@typespec/json-schema".to_string(),
        ];
        let opts = emit_options_for(&emitters, "Blog");
        assert!(opts.protobuf.is_none());
    }

    #[test]
    fn test_emit_options_derives_lowercase_package_from_namespace() {
        let emitters = vec!["@typespec/protobuf".to_string()];
        let opts = emit_options_for(&emitters, "BlogAPI");
        let pb = opts.protobuf.expect("protobuf options expected");
        assert_eq!(pb.package, "blogapi");
    }

    fn string_enum(name: &str, members: &[&str]) -> CompiledUnion {
        CompiledUnion {
            name: name.to_string(),
            variants: members
                .iter()
                .map(|m| odk_schema::CompiledUnionVariant {
                    name: m.to_string(),
                    variant_type: Some(CompiledType::Literal {
                        value: LiteralValue::String(m.to_string()),
                    }),
                })
                .collect(),
            tag: None,
            rename_all: None,
            description: None,
            extensions: Default::default(),
            source: None,
        }
    }

    #[test]
    fn test_protobuf_enums_use_integer_values() {
        let mut schema = CompiledSchema::new("Blog");
        schema.add_union(string_enum("Status", &["draft", "published"]));
        let opts = EmitOptions {
            protobuf: Some(ProtobufOptions {
                package: "blog".to_string(),
            }),
            ..Default::default()
        };

        let tsp = compiled_to_typespec_with_options(&schema, "Blog", &opts);

        // protobuf enums require explicit integer members, first == 0
        assert!(tsp.contains("enum Status {"), "got:\n{tsp}");
        assert!(tsp.contains("draft: 0,"), "got:\n{tsp}");
        assert!(tsp.contains("published: 1,"), "got:\n{tsp}");
    }

    #[test]
    fn test_string_enum_keeps_string_form_without_protobuf() {
        let mut schema = CompiledSchema::new("Blog");
        schema.add_union(string_enum("Status", &["draft", "published"]));

        let tsp = compiled_to_typespec(&schema, "Blog");

        // Without protobuf, a string enum whose member name equals its value is
        // emitted as a bare identifier, never an integer
        assert!(tsp.contains("  draft,"), "got:\n{tsp}");
        assert!(!tsp.contains("draft: 0"), "got:\n{tsp}");
    }

    #[test]
    fn test_protobuf_maps_temporal_scalars_to_string() {
        let mut schema = CompiledSchema::new("Blog");
        let mut event = CompiledModel::new("Event");
        event.add_field(CompiledField::new(
            "createdAt",
            CompiledType::Scalar {
                name: ScalarType::UtcDateTime,
            },
        ));
        schema.add_model(event);
        let opts = EmitOptions {
            protobuf: Some(ProtobufOptions {
                package: "blog".to_string(),
            }),
            ..Default::default()
        };

        let tsp = compiled_to_typespec_with_options(&schema, "Blog", &opts);

        // protobuf has no scalar for utcDateTime; it must degrade to string so
        // the emitter succeeds turnkey
        assert!(tsp.contains("@field(1) createdAt: string;"), "got:\n{tsp}");
    }

    #[test]
    fn test_temporal_scalar_unchanged_without_protobuf() {
        let mut schema = CompiledSchema::new("Blog");
        let mut event = CompiledModel::new("Event");
        event.add_field(CompiledField::new(
            "createdAt",
            CompiledType::Scalar {
                name: ScalarType::UtcDateTime,
            },
        ));
        schema.add_model(event);

        let tsp = compiled_to_typespec(&schema, "Blog");

        assert!(tsp.contains("createdAt: utcDateTime;"), "got:\n{tsp}");
    }

    #[test]
    fn test_no_protobuf_decorators_without_option() {
        let schema = protobuf_schema();

        // The plain wrapper and the explicit no-protobuf option both stay clean
        let tsp = compiled_to_typespec(&schema, "Blog");

        assert!(!tsp.contains("@package"));
        assert!(!tsp.contains("@message"));
        assert!(!tsp.contains("@field("));
        assert!(!tsp.contains("import \"@typespec/protobuf\";"));
    }
}
