//! React Query target — per-operation hooks and provider.
//!
//! Generates:
//! - `hooks.ts` — useQuery/useMutation hooks per operation
//! - `provider.tsx` — React context provider for client instance

use odk_schema::{CompiledSchema, OperationKind};
use std::collections::BTreeMap;

use super::util;

/// Generate React Query hooks and provider.
pub fn generate(schema: &CompiledSchema) -> BTreeMap<String, String> {
    let mut output = BTreeMap::new();

    if schema.operations.is_empty() {
        return output;
    }

    output.insert("hooks.ts".to_string(), generate_hooks(schema));
    output.insert("provider.tsx".to_string(), generate_provider());
    output
}

fn generate_hooks(schema: &CompiledSchema) -> String {
    let mut lines = Vec::new();

    // Collect which hook types we need
    let has_queries = schema
        .operations
        .iter()
        .any(|op| matches!(op.kind, OperationKind::Query));
    let has_mutations = schema
        .operations
        .iter()
        .any(|op| matches!(op.kind, OperationKind::Mutation));

    // React Query imports
    let mut rq_imports = Vec::new();
    if has_queries {
        rq_imports.push("useQuery");
        rq_imports.push("type UseQueryOptions");
    }
    if has_mutations {
        rq_imports.push("useMutation");
        rq_imports.push("type UseMutationOptions");
    }
    lines.push(format!(
        "import {{ {} }} from \"@tanstack/react-query\";",
        rq_imports.join(", ")
    ));

    // Client imports
    let mut client_imports = Vec::new();
    for op in &schema.operations {
        client_imports.push(format!("{}Document", op.name));
        client_imports.push(format!("type {}Result", op.name));
        if !op.variables.is_empty() {
            client_imports.push(format!("type {}Variables", op.name));
        }
    }
    lines.push(format!(
        "import {{ {} }} from \"./client\";",
        client_imports.join(", ")
    ));
    lines.push("import { useODKClient } from \"./provider\";".to_string());
    lines.push(String::new());

    // Hooks
    for op in &schema.operations {
        let hook_name = util::to_hook_name(&op.name);
        let method_name = util::to_method_name(&op.name);

        match op.kind {
            OperationKind::Query => {
                if op.variables.is_empty() {
                    lines.push(format!("export const {} = (", hook_name));
                    lines.push(format!(
                        "  options?: Omit<UseQueryOptions<{}Result>, \"queryKey\" | \"queryFn\">,",
                        op.name
                    ));
                    lines.push(") => {".to_string());
                    lines.push("  const client = useODKClient();".to_string());
                    lines.push("  return useQuery({".to_string());
                    lines.push(format!("    queryKey: [\"{}\"],", op.name));
                    lines.push(format!(
                        "    queryFn: ({{ signal }}) => client.{}({{ signal }}),",
                        method_name
                    ));
                    lines.push("    ...options,".to_string());
                    lines.push("  });".to_string());
                    lines.push("};".to_string());
                } else {
                    lines.push(format!("export const {} = (", hook_name));
                    lines.push(format!("  variables: {}Variables,", op.name));
                    lines.push(format!(
                        "  options?: Omit<UseQueryOptions<{}Result>, \"queryKey\" | \"queryFn\">,",
                        op.name
                    ));
                    lines.push(") => {".to_string());
                    lines.push("  const client = useODKClient();".to_string());
                    lines.push("  return useQuery({".to_string());
                    lines.push(format!("    queryKey: [\"{}\", variables],", op.name));
                    lines.push(format!(
                        "    queryFn: ({{ signal }}) => client.{}(variables, {{ signal }}),",
                        method_name
                    ));
                    lines.push("    ...options,".to_string());
                    lines.push("  });".to_string());
                    lines.push("};".to_string());
                }
            }
            OperationKind::Mutation => {
                if op.variables.is_empty() {
                    lines.push(format!("export const {} = (", hook_name));
                    lines.push(format!(
                        "  options?: UseMutationOptions<{}Result, Error>,",
                        op.name
                    ));
                    lines.push(") => {".to_string());
                    lines.push("  const client = useODKClient();".to_string());
                    lines.push("  return useMutation({".to_string());
                    lines.push(format!("    mutationFn: () => client.{}(),", method_name));
                    lines.push("    ...options,".to_string());
                    lines.push("  });".to_string());
                    lines.push("};".to_string());
                } else {
                    lines.push(format!("export const {} = (", hook_name));
                    lines.push(format!(
                        "  options?: UseMutationOptions<{}Result, Error, {}Variables>,",
                        op.name, op.name
                    ));
                    lines.push(") => {".to_string());
                    lines.push("  const client = useODKClient();".to_string());
                    lines.push("  return useMutation({".to_string());
                    lines.push(format!(
                        "    mutationFn: (variables: {}Variables) => client.{}(variables),",
                        op.name, method_name
                    ));
                    lines.push("    ...options,".to_string());
                    lines.push("  });".to_string());
                    lines.push("};".to_string());
                }
            }
            // Subscriptions deferred — needs transport config
            _ => {}
        }

        lines.push(String::new());
    }

    lines.join("\n")
}

fn generate_provider() -> String {
    let lines = vec![
        "import { createContext, useContext } from \"react\";".to_string(),
        "import type { Client } from \"./client\";".to_string(),
        String::new(),
        "const ODKContext = createContext<Client | null>(null);".to_string(),
        String::new(),
        "export const useODKClient = (): Client => {".to_string(),
        "  const client = useContext(ODKContext);".to_string(),
        "  if (!client) {".to_string(),
        "    throw new Error(\"useODKClient must be used within an ODKProvider\");".to_string(),
        "  }".to_string(),
        "  return client;".to_string(),
        "};".to_string(),
        String::new(),
        "export const ODKProvider = ({".to_string(),
        "  client,".to_string(),
        "  children,".to_string(),
        "}: {".to_string(),
        "  client: Client;".to_string(),
        "  children: React.ReactNode;".to_string(),
        "}) => (".to_string(),
        "  <ODKContext.Provider value={client}>{children}</ODKContext.Provider>".to_string(),
        ");".to_string(),
    ];

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
                fields: vec![FieldSelection {
                    name: "id".into(),
                    alias: None,
                    arguments: vec![],
                    selection: None,
                    directives: vec![],
                }],
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
                fields: vec![FieldSelection {
                    name: "id".into(),
                    alias: None,
                    arguments: vec![],
                    selection: None,
                    directives: vec![],
                }],
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
    fn test_generates_hooks_and_provider() {
        let output = generate(&test_schema());
        assert!(output.contains_key("hooks.ts"));
        assert!(output.contains_key("provider.tsx"));
    }

    #[test]
    fn test_hooks_has_query_hook() {
        let output = generate(&test_schema());
        let hooks = &output["hooks.ts"];
        assert!(hooks.contains("export const useGetUser"));
        assert!(hooks.contains("useQuery({"));
        assert!(hooks.contains("queryKey: [\"GetUser\", variables]"));
    }

    #[test]
    fn test_hooks_has_mutation_hook() {
        let output = generate(&test_schema());
        let hooks = &output["hooks.ts"];
        assert!(hooks.contains("export const useCreatePost"));
        assert!(hooks.contains("useMutation({"));
    }

    #[test]
    fn test_hooks_imports_from_client() {
        let output = generate(&test_schema());
        let hooks = &output["hooks.ts"];
        assert!(hooks.contains("from \"./client\""));
        assert!(hooks.contains("GetUserDocument"));
        assert!(hooks.contains("type GetUserResult"));
    }

    #[test]
    fn test_provider_has_context() {
        let output = generate(&test_schema());
        let provider = &output["provider.tsx"];
        assert!(provider.contains("ODKContext"));
        assert!(provider.contains("useODKClient"));
        assert!(provider.contains("ODKProvider"));
    }
}
