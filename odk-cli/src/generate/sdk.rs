//! SDK target — vanilla typed GraphQL client.
//!
//! Generates `client.ts` containing:
//! - gql.tada operation documents (type-inferred)
//! - Re-exported result/variable types
//! - Typed client factory with named methods

use odk_schema::CompiledSchema;
use std::collections::BTreeMap;

use super::util;

/// Generate the SDK client.
pub fn generate(schema: &CompiledSchema) -> BTreeMap<String, String> {
    let mut output = BTreeMap::new();

    if schema.operations.is_empty() {
        return output;
    }

    let content = generate_client(schema);
    output.insert("client.ts".to_string(), content);
    output
}

fn generate_client(schema: &CompiledSchema) -> String {
    let mut lines = Vec::new();

    // Imports
    lines
        .push("import { graphql, type ResultOf, type VariablesOf } from \"gql.tada\";".to_string());
    lines.push("import { GraphQLClient } from \"graphql-request\";".to_string());
    lines.push(String::new());

    // Operation documents
    for op in &schema.operations {
        let doc_text = util::reconstruct_operation(
            &op.name,
            &op.kind,
            &op.variables,
            &op.selection,
            &schema.fragments,
        );

        lines.push(format!("export const {}Document = graphql(`", op.name));
        for doc_line in doc_text.lines() {
            lines.push(format!("  {}", doc_line));
        }
        lines.push("`);".to_string());
        lines.push(String::new());
    }

    // Type re-exports
    for op in &schema.operations {
        lines.push(format!(
            "export type {}Result = ResultOf<typeof {}Document>;",
            op.name, op.name
        ));
        if !op.variables.is_empty() {
            lines.push(format!(
                "export type {}Variables = VariablesOf<typeof {}Document>;",
                op.name, op.name
            ));
        }
    }
    lines.push(String::new());

    // Client factory
    lines.push("export const createClient = (url: string, options?: RequestInit) => {".to_string());
    lines.push("  const gql = new GraphQLClient(url, options);".to_string());
    lines.push("  return {".to_string());

    for op in &schema.operations {
        let method = util::to_method_name(&op.name);
        if op.variables.is_empty() {
            lines.push(format!(
                "    {}: () => gql.request({}Document),",
                method, op.name
            ));
        } else {
            lines.push(format!(
                "    {}: (variables: {}Variables) => gql.request({}Document, variables),",
                method, op.name, op.name
            ));
        }
    }

    lines.push("  };".to_string());
    lines.push("};".to_string());
    lines.push(String::new());

    // Export client type
    lines.push("export type Client = ReturnType<typeof createClient>;".to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use odk_schema::*;

    fn test_schema() -> CompiledSchema {
        let mut schema = CompiledSchema::new("TestAPI");

        schema.add_operation(CompiledOperation {
            name: "GetUser".into(),
            kind: OperationKind::Query,
            input: None,
            output: CompiledType::Ref {
                name: "User".into(),
            },
            variables: vec![Variable {
                name: "id".into(),
                variable_type: CompiledType::Scalar {
                    name: ScalarType::String,
                },
                required: true,
                default_value: None,
                description: None,
            }],
            selection: Some(SelectionSet {
                fields: vec![
                    FieldSelection {
                        name: "id".into(),
                        alias: None,
                        arguments: vec![],
                        selection: None,
                        directives: vec![],
                    },
                    FieldSelection {
                        name: "name".into(),
                        alias: None,
                        arguments: vec![],
                        selection: None,
                        directives: vec![],
                    },
                ],
                fragment_spreads: vec![],
                inline_fragments: vec![],
            }),
            description: None,
            extensions: Extensions::default(),
            directives: vec![],
        });

        schema.add_operation(CompiledOperation {
            name: "CreatePost".into(),
            kind: OperationKind::Mutation,
            input: None,
            output: CompiledType::Ref {
                name: "Post".into(),
            },
            variables: vec![Variable {
                name: "input".into(),
                variable_type: CompiledType::Ref {
                    name: "CreatePostInput".into(),
                },
                required: true,
                default_value: None,
                description: None,
            }],
            selection: Some(SelectionSet {
                fields: vec![
                    FieldSelection {
                        name: "id".into(),
                        alias: None,
                        arguments: vec![],
                        selection: None,
                        directives: vec![],
                    },
                    FieldSelection {
                        name: "title".into(),
                        alias: None,
                        arguments: vec![],
                        selection: None,
                        directives: vec![],
                    },
                ],
                fragment_spreads: vec![],
                inline_fragments: vec![],
            }),
            description: None,
            extensions: Extensions::default(),
            directives: vec![],
        });

        schema
    }

    #[test]
    fn test_generates_client_file() {
        let schema = test_schema();
        let output = generate(&schema);
        assert!(output.contains_key("client.ts"));
    }

    #[test]
    fn test_client_has_imports() {
        let schema = test_schema();
        let output = generate(&schema);
        let client = &output["client.ts"];
        assert!(
            client
                .contains("import { graphql, type ResultOf, type VariablesOf } from \"gql.tada\"")
        );
        assert!(client.contains("import { GraphQLClient } from \"graphql-request\""));
    }

    #[test]
    fn test_client_has_operation_documents() {
        let schema = test_schema();
        let output = generate(&schema);
        let client = &output["client.ts"];
        assert!(client.contains("export const GetUserDocument = graphql(`"));
        assert!(client.contains("export const CreatePostDocument = graphql(`"));
        assert!(client.contains("query GetUser($id: String!)"));
        assert!(client.contains("mutation CreatePost($input: CreatePostInput!)"));
    }

    #[test]
    fn test_client_has_type_exports() {
        let schema = test_schema();
        let output = generate(&schema);
        let client = &output["client.ts"];
        assert!(client.contains("export type GetUserResult = ResultOf<typeof GetUserDocument>"));
        assert!(
            client.contains("export type GetUserVariables = VariablesOf<typeof GetUserDocument>")
        );
        assert!(
            client.contains("export type CreatePostResult = ResultOf<typeof CreatePostDocument>")
        );
    }

    #[test]
    fn test_client_has_factory_methods() {
        let schema = test_schema();
        let output = generate(&schema);
        let client = &output["client.ts"];
        assert!(client.contains("getUser: (variables: GetUserVariables)"));
        assert!(client.contains("createPost: (variables: CreatePostVariables)"));
    }

    #[test]
    fn test_empty_operations_no_output() {
        let schema = CompiledSchema::new("Empty");
        let output = generate(&schema);
        assert!(output.is_empty());
    }
}
