//! GraphQL schema and operation document loader
//!
//! Parses GraphQL SDL files (.graphql) into CompiledSchema types and
//! GraphQL operation documents into CompiledOperations.

use apollo_parser::cst::CstNode;
use apollo_parser::Parser;
use odk_schema::{
    CompiledField, CompiledModel, CompiledOperation, CompiledSchema, CompiledType,
    CompiledUnion, CompiledUnionVariant, Directive, Extensions, FieldSelection,
    FragmentDefinition, FragmentSpread, InlineFragment, LiteralValue, OperationKind,
    ScalarType, SelectionSet, Variable,
};

/// Error type for GraphQL loading.
#[derive(Debug)]
pub enum GraphqlLoadError {
    Io(std::io::Error),
    Parse(Vec<String>),
    Validation(String),
}

impl std::fmt::Display for GraphqlLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphqlLoadError::Io(e) => write!(f, "IO error: {}", e),
            GraphqlLoadError::Parse(errors) => {
                write!(f, "GraphQL parse errors: {}", errors.join("; "))
            }
            GraphqlLoadError::Validation(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for GraphqlLoadError {}

impl From<std::io::Error> for GraphqlLoadError {
    fn from(e: std::io::Error) -> Self {
        GraphqlLoadError::Io(e)
    }
}

/// Map a GraphQL built-in scalar name to a CompiledType.
fn map_graphql_scalar(name: &str) -> Option<CompiledType> {
    match name {
        "String" => Some(CompiledType::Scalar {
            name: ScalarType::String,
        }),
        "Int" => Some(CompiledType::Scalar {
            name: ScalarType::Int32,
        }),
        "Float" => Some(CompiledType::Scalar {
            name: ScalarType::Float64,
        }),
        "Boolean" => Some(CompiledType::Scalar {
            name: ScalarType::Boolean,
        }),
        "ID" => {
            // ID maps to String with a format extension
            Some(CompiledType::Scalar {
                name: ScalarType::String,
            })
        }
        _ => None,
    }
}

/// Map a GraphQL type AST node to a CompiledType.
///
/// Handles named types, list types, and non-null wrappers.
/// In the IR, fields are non-nullable by default, and `optional: true`
/// marks nullable fields. GraphQL's `Type!` means non-null (IR default),
/// and `Type` (without `!`) means nullable.
pub fn map_graphql_type(ty: &apollo_parser::cst::Type) -> (CompiledType, bool) {
    use apollo_parser::cst::Type;

    match ty {
        Type::NamedType(named) => {
            let name = named
                .name()
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            let compiled = map_graphql_scalar(&name)
                .unwrap_or_else(|| CompiledType::Ref { name });
            // Named type without `!` is nullable in GraphQL
            (compiled, true)
        }
        Type::ListType(list) => {
            if let Some(inner) = list.ty() {
                let (inner_type, _inner_nullable) = map_graphql_type(&inner);
                (CompiledType::Array { items: Box::new(inner_type), length: None }, true)
            } else {
                (CompiledType::Scalar { name: ScalarType::Unknown }, true)
            }
        }
        Type::NonNullType(non_null) => {
            if let Some(named) = non_null.named_type() {
                let name = named
                    .name()
                    .map(|n| n.text().to_string())
                    .unwrap_or_default();
                let compiled = map_graphql_scalar(&name)
                    .unwrap_or_else(|| CompiledType::Ref { name });
                (compiled, false)
            } else if let Some(list) = non_null.list_type() {
                if let Some(inner) = list.ty() {
                    let (inner_type, _) = map_graphql_type(&inner);
                    (CompiledType::Array { items: Box::new(inner_type), length: None }, false)
                } else {
                    (CompiledType::Scalar { name: ScalarType::Unknown }, false)
                }
            } else {
                (CompiledType::Scalar { name: ScalarType::Unknown }, false)
            }
        }
    }
}

/// Load a GraphQL SDL schema file into a CompiledSchema.
pub fn load_graphql_schema(
    path: &std::path::Path,
) -> Result<CompiledSchema, GraphqlLoadError> {
    let content = std::fs::read_to_string(path)?;
    let file_name = path.to_string_lossy().to_string();
    parse_graphql_sdl(&content, &file_name)
}

/// Parse a GraphQL SDL string into a CompiledSchema.
pub fn parse_graphql_sdl(
    source: &str,
    file_name: &str,
) -> Result<CompiledSchema, GraphqlLoadError> {
    let parser = Parser::new(source);
    let cst = parser.parse();

    // Collect parse errors
    let errors: Vec<String> = cst.errors().map(|e| e.message().to_string()).collect();
    if !errors.is_empty() {
        return Err(GraphqlLoadError::Parse(errors));
    }

    let doc = cst.document();

    // Derive schema name from file name
    let schema_name = std::path::Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Schema")
        .to_string();

    let mut schema = CompiledSchema::new(&schema_name);

    for def in doc.definitions() {
        use apollo_parser::cst::Definition;

        match def {
            Definition::ObjectTypeDefinition(obj) => {
                let model = convert_object_type(&obj)?;
                schema.add_model(model);
            }
            Definition::InputObjectTypeDefinition(input) => {
                let model = convert_input_type(&input)?;
                schema.add_model(model);
            }
            Definition::InterfaceTypeDefinition(iface) => {
                let model = convert_interface_type(&iface)?;
                schema.add_model(model);
            }
            Definition::EnumTypeDefinition(en) => {
                let union = convert_enum_type(&en)?;
                schema.add_union(union);
            }
            Definition::UnionTypeDefinition(un) => {
                let union = convert_union_type(&un)?;
                schema.add_union(union);
            }
            Definition::ScalarTypeDefinition(sc) => {
                // Custom scalars become models with a single `value` field
                // and a `graphql_custom_scalar` extension
                let name = sc
                    .name()
                    .map(|n| n.text().to_string())
                    .unwrap_or_default();
                let description = sc.description().and_then(|d| {
                    d.string_value()
                        .map(|s| s.source_string().trim_matches('"').to_string())
                });
                let mut model = CompiledModel::new(&name);
                model.description = description;
                model.extensions.add_unknown(
                    "graphql_custom_scalar".to_string(),
                    serde_json::json!(true),
                );
                schema.add_model(model);
            }
            // Skip schema definition, directive definitions, extensions for now
            _ => {}
        }
    }

    Ok(schema)
}

/// Convert a GraphQL object type to a CompiledModel.
fn convert_object_type(
    obj: &apollo_parser::cst::ObjectTypeDefinition,
) -> Result<CompiledModel, GraphqlLoadError> {
    let name = obj
        .name()
        .map(|n| n.text().to_string())
        .unwrap_or_default();
    let description = obj.description().and_then(|d| {
        d.string_value()
            .map(|s| s.source_string().trim_matches('"').to_string())
    });

    let mut model = CompiledModel::new(&name);
    model.description = description;

    // Add implemented interfaces as extension
    if let Some(impls) = obj.implements_interfaces() {
        let interfaces: Vec<String> = impls
            .named_types()
            .filter_map(|n| n.name().map(|name| name.text().to_string()))
            .collect();
        if !interfaces.is_empty() {
            model.extensions.add_unknown(
                "graphql_implements".to_string(),
                serde_json::json!(interfaces),
            );
        }
    }

    if let Some(fields_def) = obj.fields_definition() {
        for field_def in fields_def.field_definitions() {
            let field = convert_field_definition(&field_def)?;
            model.add_field(field);
        }
    }

    Ok(model)
}

/// Convert a GraphQL input type to a CompiledModel with `graphql_input` extension.
fn convert_input_type(
    input: &apollo_parser::cst::InputObjectTypeDefinition,
) -> Result<CompiledModel, GraphqlLoadError> {
    let name = input
        .name()
        .map(|n| n.text().to_string())
        .unwrap_or_default();
    let description = input.description().and_then(|d| {
        d.string_value()
            .map(|s| s.source_string().trim_matches('"').to_string())
    });

    let mut model = CompiledModel::new(&name);
    model.description = description;
    model
        .extensions
        .add_unknown("graphql_input".to_string(), serde_json::json!(true));

    if let Some(fields_def) = input.input_fields_definition() {
        for input_val in fields_def.input_value_definitions() {
            let field_name = input_val
                .name()
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            let field_desc = input_val.description().and_then(|d| {
                d.string_value()
                    .map(|s| s.source_string().trim_matches('"').to_string())
            });

            if let Some(ty) = input_val.ty() {
                let (compiled_type, nullable) = map_graphql_type(&ty);
                let mut field = CompiledField::new(&field_name, compiled_type);
                if nullable {
                    field.optional = Some(true);
                }
                field.description = field_desc;
                model.add_field(field);
            }
        }
    }

    Ok(model)
}

/// Convert a GraphQL interface to a CompiledModel with `graphql_interface` extension.
fn convert_interface_type(
    iface: &apollo_parser::cst::InterfaceTypeDefinition,
) -> Result<CompiledModel, GraphqlLoadError> {
    let name = iface
        .name()
        .map(|n| n.text().to_string())
        .unwrap_or_default();
    let description = iface.description().and_then(|d| {
        d.string_value()
            .map(|s| s.source_string().trim_matches('"').to_string())
    });

    let mut model = CompiledModel::new(&name);
    model.description = description;
    model
        .extensions
        .add_unknown("graphql_interface".to_string(), serde_json::json!(true));

    if let Some(fields_def) = iface.fields_definition() {
        for field_def in fields_def.field_definitions() {
            let field = convert_field_definition(&field_def)?;
            model.add_field(field);
        }
    }

    Ok(model)
}

/// Convert a GraphQL field definition to a CompiledField.
fn convert_field_definition(
    field_def: &apollo_parser::cst::FieldDefinition,
) -> Result<CompiledField, GraphqlLoadError> {
    let name = field_def
        .name()
        .map(|n| n.text().to_string())
        .unwrap_or_default();
    let description = field_def.description().and_then(|d| {
        d.string_value()
            .map(|s| s.source_string().trim_matches('"').to_string())
    });

    let (compiled_type, nullable) = if let Some(ty) = field_def.ty() {
        map_graphql_type(&ty)
    } else {
        (
            CompiledType::Scalar {
                name: ScalarType::Unknown,
            },
            true,
        )
    };

    let mut field = CompiledField::new(&name, compiled_type);
    if nullable {
        field.optional = Some(true);
    }
    field.description = description;

    // Store field arguments as extension metadata (for resolver-aware emitters)
    if let Some(args_def) = field_def.arguments_definition() {
        let args: Vec<serde_json::Value> = args_def
            .input_value_definitions()
            .filter_map(|arg| {
                let arg_name = arg.name().map(|n| n.text().to_string())?;
                let arg_type = arg.ty().map(|t| format!("{}", t.source_string()))?;
                Some(serde_json::json!({
                    "name": arg_name,
                    "type": arg_type,
                }))
            })
            .collect();
        if !args.is_empty() {
            field.extensions.add_unknown(
                "graphql_arguments".to_string(),
                serde_json::json!(args),
            );
        }
    }

    Ok(field)
}

/// Convert a GraphQL enum to a CompiledUnion with literal string variants.
fn convert_enum_type(
    en: &apollo_parser::cst::EnumTypeDefinition,
) -> Result<CompiledUnion, GraphqlLoadError> {
    let name = en
        .name()
        .map(|n| n.text().to_string())
        .unwrap_or_default();
    let description = en.description().and_then(|d| {
        d.string_value()
            .map(|s| s.source_string().trim_matches('"').to_string())
    });

    let mut variants = Vec::new();
    if let Some(vals) = en.enum_values_definition() {
        for val in vals.enum_value_definitions() {
            let val_name = val
                .enum_value()
                .map(|v| v.name().map(|n| n.text().to_string()).unwrap_or_default())
                .unwrap_or_default();
            variants.push(CompiledUnionVariant {
                name: val_name.clone(),
                variant_type: Some(CompiledType::Literal {
                    value: LiteralValue::String(val_name),
                }),
            });
        }
    }

    Ok(CompiledUnion {
        name,
        variants,
        tag: None,
        rename_all: None,
        description,
        extensions: Extensions::default(),
        source: None,
    })
}

/// Convert a GraphQL union to a CompiledUnion with ref variants.
fn convert_union_type(
    un: &apollo_parser::cst::UnionTypeDefinition,
) -> Result<CompiledUnion, GraphqlLoadError> {
    let name = un
        .name()
        .map(|n| n.text().to_string())
        .unwrap_or_default();
    let description = un.description().and_then(|d| {
        d.string_value()
            .map(|s| s.source_string().trim_matches('"').to_string())
    });

    let mut variants = Vec::new();
    if let Some(members) = un.union_member_types() {
        for member in members.named_types() {
            let member_name = member
                .name()
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            variants.push(CompiledUnionVariant {
                name: member_name.clone(),
                variant_type: Some(CompiledType::Ref {
                    name: member_name,
                }),
            });
        }
    }

    Ok(CompiledUnion {
        name,
        variants,
        tag: None,
        rename_all: None,
        description,
        extensions: Extensions::default(),
        source: None,
    })
}

/// Load GraphQL operation documents from files matching a glob pattern.
///
/// Returns a vector of operations and fragment definitions found across
/// all matched files.
pub fn load_graphql_operations(
    pattern: &str,
) -> Result<(Vec<CompiledOperation>, Vec<FragmentDefinition>), GraphqlLoadError> {
    let paths = glob::glob(pattern).map_err(|e| {
        GraphqlLoadError::Validation(format!("Invalid glob pattern '{}': {}", pattern, e))
    })?;

    let mut operations = Vec::new();
    let mut fragments = Vec::new();

    for entry in paths {
        let path = entry.map_err(|e| GraphqlLoadError::Io(e.into_error()))?;
        let content = std::fs::read_to_string(&path)?;
        let (mut ops, mut frags) =
            parse_graphql_operations(&content, &path.to_string_lossy())?;
        operations.append(&mut ops);
        fragments.append(&mut frags);
    }

    Ok((operations, fragments))
}

/// Parse a GraphQL operation document string.
pub fn parse_graphql_operations(
    source: &str,
    file_name: &str,
) -> Result<(Vec<CompiledOperation>, Vec<FragmentDefinition>), GraphqlLoadError> {
    let parser = Parser::new(source);
    let cst = parser.parse();

    let errors: Vec<String> = cst.errors().map(|e| e.message().to_string()).collect();
    if !errors.is_empty() {
        return Err(GraphqlLoadError::Parse(errors));
    }

    let doc = cst.document();
    let mut operations = Vec::new();
    let mut fragments = Vec::new();

    for def in doc.definitions() {
        use apollo_parser::cst::Definition;

        match def {
            Definition::OperationDefinition(op_def) => {
                let op = convert_operation(&op_def, file_name)?;
                operations.push(op);
            }
            Definition::FragmentDefinition(frag_def) => {
                let frag = convert_fragment_definition(&frag_def)?;
                fragments.push(frag);
            }
            _ => {
                // Operation documents may also contain type definitions
                // (common in colocated .graphql files), skip them here
            }
        }
    }

    Ok((operations, fragments))
}

/// Convert a GraphQL operation definition to a CompiledOperation.
fn convert_operation(
    op_def: &apollo_parser::cst::OperationDefinition,
    _file_name: &str,
) -> Result<CompiledOperation, GraphqlLoadError> {
    // Determine operation kind
    let kind = match op_def.operation_type() {
        Some(op_type) => {
            if op_type.query_token().is_some() {
                OperationKind::Query
            } else if op_type.mutation_token().is_some() {
                OperationKind::Mutation
            } else if op_type.subscription_token().is_some() {
                OperationKind::Subscription
            } else {
                OperationKind::Query
            }
        }
        None => OperationKind::Query, // Shorthand query
    };

    // Get operation name
    let name = op_def
        .name()
        .map(|n| n.text().to_string())
        .unwrap_or_else(|| "Anonymous".to_string());

    // Parse variables
    let variables = if let Some(var_defs) = op_def.variable_definitions() {
        var_defs
            .variable_definitions()
            .filter_map(|var_def| {
                let var_name = var_def
                    .variable()
                    .and_then(|v| v.name())
                    .map(|n| n.text().to_string())?;
                let (var_type, nullable) = var_def
                    .ty()
                    .map(|ty| map_graphql_type(&ty))
                    .unwrap_or((
                        CompiledType::Scalar {
                            name: ScalarType::Unknown,
                        },
                        true,
                    ));
                Some(Variable {
                    name: var_name,
                    variable_type: var_type,
                    required: !nullable,
                    default_value: var_def.default_value().map(|_| serde_json::json!(null)),
                    description: None,
                })
            })
            .collect()
    } else {
        vec![]
    };

    // Parse selection set
    let selection = op_def
        .selection_set()
        .map(|sel| convert_selection_set(&sel));

    // Parse directives
    let directives = op_def
        .directives()
        .map(|dirs| convert_directives(&dirs))
        .unwrap_or_default();

    // Build a synthetic output type from the selection set
    // (actual type resolution requires the schema, so use Unknown for now)
    let output = CompiledType::Scalar {
        name: ScalarType::Unknown,
    };

    Ok(CompiledOperation {
        name,
        kind,
        input: None,
        output,
        variables,
        selection,
        description: None,
        extensions: Extensions::default(),
        directives,
    })
}

/// Convert a selection set CST node to a SelectionSet IR node.
fn convert_selection_set(sel_set: &apollo_parser::cst::SelectionSet) -> SelectionSet {
    let mut fields = Vec::new();
    let mut fragment_spreads = Vec::new();
    let mut inline_fragments = Vec::new();

    for selection in sel_set.selections() {
        use apollo_parser::cst::Selection;

        match selection {
            Selection::Field(field) => {
                let name = field
                    .name()
                    .map(|n| n.text().to_string())
                    .unwrap_or_default();
                let alias = field.alias().and_then(|a| a.name().map(|n| n.text().to_string()));

                let arguments: Vec<(String, CompiledType)> = field
                    .arguments()
                    .map(|args| {
                        args.arguments()
                            .filter_map(|arg| {
                                let arg_name = arg.name().map(|n| n.text().to_string())?;
                                // Argument values in selection sets reference variables
                                // or are literals; store as Unknown type placeholder
                                Some((
                                    arg_name,
                                    CompiledType::Scalar {
                                        name: ScalarType::Unknown,
                                    },
                                ))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let nested = field.selection_set().map(|s| convert_selection_set(&s));

                let directives = field
                    .directives()
                    .map(|dirs| convert_directives(&dirs))
                    .unwrap_or_default();

                fields.push(FieldSelection {
                    name,
                    alias,
                    arguments,
                    selection: nested,
                    directives,
                });
            }
            Selection::FragmentSpread(spread) => {
                let name = spread
                    .fragment_name()
                    .and_then(|n| n.name())
                    .map(|n| n.text().to_string())
                    .unwrap_or_default();
                let directives = spread
                    .directives()
                    .map(|dirs| convert_directives(&dirs))
                    .unwrap_or_default();
                fragment_spreads.push(FragmentSpread { name, directives });
            }
            Selection::InlineFragment(inline) => {
                let type_condition = inline
                    .type_condition()
                    .and_then(|tc| tc.named_type())
                    .and_then(|nt| nt.name())
                    .map(|n| n.text().to_string());
                let selection = inline
                    .selection_set()
                    .map(|s| convert_selection_set(&s))
                    .unwrap_or(SelectionSet {
                        fields: vec![],
                        fragment_spreads: vec![],
                        inline_fragments: vec![],
                    });
                let directives = inline
                    .directives()
                    .map(|dirs| convert_directives(&dirs))
                    .unwrap_or_default();
                inline_fragments.push(InlineFragment {
                    type_condition,
                    selection,
                    directives,
                });
            }
        }
    }

    SelectionSet {
        fields,
        fragment_spreads,
        inline_fragments,
    }
}

/// Convert GraphQL directives to IR Directive structs.
fn convert_directives(dirs: &apollo_parser::cst::Directives) -> Vec<Directive> {
    dirs.directives()
        .filter_map(|dir| {
            let name = dir.name().map(|n| n.text().to_string())?;
            let arguments: Vec<(String, serde_json::Value)> = dir
                .arguments()
                .map(|args| {
                    args.arguments()
                        .filter_map(|arg| {
                            let arg_name = arg.name().map(|n| n.text().to_string())?;
                            // Convert value to JSON
                            let value = arg
                                .value()
                                .map(|v| serde_json::json!(v.source_string()))
                                .unwrap_or(serde_json::json!(null));
                            Some((arg_name, value))
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(Directive { name, arguments })
        })
        .collect()
}

/// Convert a GraphQL fragment definition to a FragmentDefinition.
fn convert_fragment_definition(
    frag: &apollo_parser::cst::FragmentDefinition,
) -> Result<FragmentDefinition, GraphqlLoadError> {
    let name = frag
        .fragment_name()
        .and_then(|n| n.name())
        .map(|n| n.text().to_string())
        .unwrap_or_default();

    let type_condition = frag
        .type_condition()
        .and_then(|tc| tc.named_type())
        .and_then(|nt| nt.name())
        .map(|n| n.text().to_string())
        .unwrap_or_default();

    let selection = frag
        .selection_set()
        .map(|s| convert_selection_set(&s))
        .unwrap_or(SelectionSet {
            fields: vec![],
            fragment_spreads: vec![],
            inline_fragments: vec![],
        });

    let directives = frag
        .directives()
        .map(|dirs| convert_directives(&dirs))
        .unwrap_or_default();

    Ok(FragmentDefinition {
        name,
        type_condition,
        selection,
        directives,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_object_type() {
        let sdl = r#"
            type User {
                id: ID!
                name: String!
                email: String
                age: Int
            }
        "#;

        let schema = parse_graphql_sdl(sdl, "test.graphql").unwrap();
        assert_eq!(schema.models.len(), 1);

        let user = &schema.models[0];
        assert_eq!(user.name, "User");
        assert_eq!(user.fields.len(), 4);

        // ID! -> String, required
        let id_field = user.fields.iter().find(|f| f.name == "id").unwrap();
        assert!(id_field.optional.is_none() || !id_field.optional.unwrap());

        // String (nullable) -> optional
        let email_field = user.fields.iter().find(|f| f.name == "email").unwrap();
        assert_eq!(email_field.optional, Some(true));
    }

    #[test]
    fn test_parse_input_type() {
        let sdl = r#"
            input CreateUserInput {
                name: String!
                email: String!
                bio: String
            }
        "#;

        let schema = parse_graphql_sdl(sdl, "test.graphql").unwrap();
        assert_eq!(schema.models.len(), 1);

        let input = &schema.models[0];
        assert_eq!(input.name, "CreateUserInput");
        assert!(!input.extensions.unknown.is_empty());
        assert_eq!(
            input.extensions.unknown.get("graphql_input"),
            Some(&serde_json::json!(true))
        );
    }

    #[test]
    fn test_parse_enum() {
        let sdl = r#"
            enum Status {
                ACTIVE
                INACTIVE
                PENDING
            }
        "#;

        let schema = parse_graphql_sdl(sdl, "test.graphql").unwrap();
        assert_eq!(schema.unions.len(), 1);

        let status = &schema.unions[0];
        assert_eq!(status.name, "Status");
        assert_eq!(status.variants.len(), 3);
        assert_eq!(status.variants[0].name, "ACTIVE");
    }

    #[test]
    fn test_parse_union() {
        let sdl = r#"
            type Cat { name: String! }
            type Dog { name: String! }
            union Animal = Cat | Dog
        "#;

        let schema = parse_graphql_sdl(sdl, "test.graphql").unwrap();
        assert_eq!(schema.models.len(), 2);
        assert_eq!(schema.unions.len(), 1);

        let animal = &schema.unions[0];
        assert_eq!(animal.name, "Animal");
        assert_eq!(animal.variants.len(), 2);

        // Union variants should be Ref types
        match &animal.variants[0].variant_type {
            Some(CompiledType::Ref { name }) => assert_eq!(name, "Cat"),
            other => panic!("Expected Ref variant, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_interface() {
        let sdl = r#"
            interface Node {
                id: ID!
            }
        "#;

        let schema = parse_graphql_sdl(sdl, "test.graphql").unwrap();
        assert_eq!(schema.models.len(), 1);

        let node = &schema.models[0];
        assert_eq!(node.name, "Node");
        assert_eq!(
            node.extensions.unknown.get("graphql_interface"),
            Some(&serde_json::json!(true))
        );
    }

    #[test]
    fn test_parse_array_types() {
        let sdl = r#"
            type User {
                tags: [String!]!
                posts: [Post]
            }
        "#;

        let schema = parse_graphql_sdl(sdl, "test.graphql").unwrap();
        let user = &schema.models[0];

        let tags = user.fields.iter().find(|f| f.name == "tags").unwrap();
        match &tags.field_type {
            CompiledType::Array { .. } => {}
            other => panic!("Expected Array type for tags, got {:?}", other),
        }
        // [String!]! -> required array
        assert!(tags.optional.is_none() || !tags.optional.unwrap());

        let posts = user.fields.iter().find(|f| f.name == "posts").unwrap();
        match &posts.field_type {
            CompiledType::Array { .. } => {}
            other => panic!("Expected Array type for posts, got {:?}", other),
        }
        // [Post] (nullable) -> optional
        assert_eq!(posts.optional, Some(true));
    }

    #[test]
    fn test_parse_query_operation() {
        let doc = r#"
            query GetUser($id: ID!) {
                user(id: $id) {
                    id
                    name
                    email
                }
            }
        "#;

        let (ops, frags) = parse_graphql_operations(doc, "test.graphql").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(frags.len(), 0);

        let op = &ops[0];
        assert_eq!(op.name, "GetUser");
        assert!(matches!(op.kind, OperationKind::Query));
        assert_eq!(op.variables.len(), 1);
        assert_eq!(op.variables[0].name, "id");
        assert!(op.variables[0].required);

        let sel = op.selection.as_ref().unwrap();
        assert_eq!(sel.fields.len(), 1);
        assert_eq!(sel.fields[0].name, "user");

        // Nested selection
        let user_sel = sel.fields[0].selection.as_ref().unwrap();
        assert_eq!(user_sel.fields.len(), 3);
    }

    #[test]
    fn test_parse_mutation_operation() {
        let doc = r#"
            mutation CreateUser($input: CreateUserInput!) {
                createUser(input: $input) {
                    id
                    name
                }
            }
        "#;

        let (ops, _) = parse_graphql_operations(doc, "test.graphql").unwrap();
        assert_eq!(ops.len(), 1);

        let op = &ops[0];
        assert_eq!(op.name, "CreateUser");
        assert!(matches!(op.kind, OperationKind::Mutation));
    }

    #[test]
    fn test_parse_fragment_definition() {
        let doc = r#"
            fragment UserFields on User {
                id
                name
                email
            }

            query GetUser($id: ID!) {
                user(id: $id) {
                    ...UserFields
                }
            }
        "#;

        let (ops, frags) = parse_graphql_operations(doc, "test.graphql").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(frags.len(), 1);

        let frag = &frags[0];
        assert_eq!(frag.name, "UserFields");
        assert_eq!(frag.type_condition, "User");
        assert_eq!(frag.selection.fields.len(), 3);

        // Verify the operation has a fragment spread
        let sel = ops[0].selection.as_ref().unwrap();
        assert_eq!(sel.fields[0].name, "user");
        let user_sel = sel.fields[0].selection.as_ref().unwrap();
        assert_eq!(user_sel.fragment_spreads.len(), 1);
        assert_eq!(user_sel.fragment_spreads[0].name, "UserFields");
    }

    #[test]
    fn test_parse_inline_fragment() {
        let doc = r#"
            query SearchResults($query: String!) {
                search(query: $query) {
                    ... on User {
                        name
                        email
                    }
                    ... on Post {
                        title
                        body
                    }
                }
            }
        "#;

        let (ops, _) = parse_graphql_operations(doc, "test.graphql").unwrap();
        let sel = ops[0].selection.as_ref().unwrap();
        let search_sel = sel.fields[0].selection.as_ref().unwrap();
        assert_eq!(search_sel.inline_fragments.len(), 2);
        assert_eq!(
            search_sel.inline_fragments[0].type_condition,
            Some("User".to_string())
        );
        assert_eq!(
            search_sel.inline_fragments[1].type_condition,
            Some("Post".to_string())
        );
    }

    #[test]
    fn test_parse_field_alias() {
        let doc = r#"
            query GetUsers {
                activeUsers: users(status: ACTIVE) {
                    id
                }
            }
        "#;

        let (ops, _) = parse_graphql_operations(doc, "test.graphql").unwrap();
        let sel = ops[0].selection.as_ref().unwrap();
        assert_eq!(sel.fields[0].alias, Some("activeUsers".to_string()));
        assert_eq!(sel.fields[0].name, "users");
    }

    #[test]
    fn test_parse_directive_on_field() {
        let doc = r#"
            query GetUser($id: ID!, $includeEmail: Boolean!) {
                user(id: $id) {
                    id
                    email @include(if: $includeEmail)
                }
            }
        "#;

        let (ops, _) = parse_graphql_operations(doc, "test.graphql").unwrap();
        let sel = ops[0].selection.as_ref().unwrap();
        let user_sel = sel.fields[0].selection.as_ref().unwrap();
        let email_field = user_sel
            .fields
            .iter()
            .find(|f| f.name == "email")
            .unwrap();
        assert_eq!(email_field.directives.len(), 1);
        assert_eq!(email_field.directives[0].name, "include");
    }
}
