//! Integration test: GraphQL schema + operations → generated client artifacts

use odk_cli::config::{GenerateConfig, McpGenerateConfig, McpToolConfig, McpToolParameter};
use odk_cli::generate::{self, Target};
use odk_cli::loaders::graphql;

#[test]
fn test_full_generate_pipeline() {
    let sdl = r#"
        type Query {
            user(id: ID!): User
            users: [User!]!
        }

        type Mutation {
            createUser(input: CreateUserInput!): User!
        }

        type User {
            id: ID!
            name: String!
            email: String
        }

        input CreateUserInput {
            name: String!
            email: String!
        }
    "#;

    let ops_doc = r#"
        query GetUser($id: ID!) {
            user(id: $id) {
                id
                name
                email
            }
        }

        query ListUsers {
            users {
                id
                name
            }
        }

        mutation CreateUser($input: CreateUserInput!) {
            createUser(input: $input) {
                id
                name
            }
        }
    "#;

    // Parse
    let mut schema = graphql::parse_graphql_sdl(sdl, "schema.graphql").unwrap();
    let (ops, frags) = graphql::parse_graphql_operations(ops_doc, "ops.graphql").unwrap();
    for op in ops {
        schema.add_operation(op);
    }
    for frag in frags {
        schema.add_fragment(frag);
    }

    // Generate all targets
    let targets = vec![Target::Sdk, Target::ReactQuery, Target::Msw];
    let output = generate::generate(&schema, &targets, None);

    // SDK
    assert!(output.contains_key("client.ts"));
    let client = &output["client.ts"];
    assert!(client.contains("GetUserDocument"));
    assert!(client.contains("ListUsersDocument"));
    assert!(client.contains("CreateUserDocument"));
    assert!(client.contains("createClient"));
    assert!(client.contains("getUser:"));
    assert!(client.contains("listUsers:"));
    assert!(client.contains("createUser:"));

    // React Query
    assert!(output.contains_key("hooks.ts"));
    let hooks = &output["hooks.ts"];
    assert!(hooks.contains("useGetUser"));
    assert!(hooks.contains("useListUsers"));
    assert!(hooks.contains("useCreateUser"));
    assert!(hooks.contains("useQuery"));
    assert!(hooks.contains("useMutation"));

    assert!(output.contains_key("provider.tsx"));
    let provider = &output["provider.tsx"];
    assert!(provider.contains("ODKProvider"));

    // MSW
    assert!(output.contains_key("mocks.ts"));
    let mocks = &output["mocks.ts"];
    assert!(mocks.contains("mockGetUser"));
    assert!(mocks.contains("mockListUsers"));
    assert!(mocks.contains("mockCreateUser"));
    assert!(mocks.contains("mockGetUserData"));
    assert!(mocks.contains("mockGetUserError"));
    assert!(mocks.contains("allHandlers"));
}

#[test]
fn test_sdk_only() {
    let sdl = r#"
        type Query { user(id: ID!): User }
        type User { id: ID! name: String! }
    "#;
    let ops = r#"
        query GetUser($id: ID!) { user(id: $id) { id name } }
    "#;

    let mut schema = graphql::parse_graphql_sdl(sdl, "s.graphql").unwrap();
    let (ops, frags) = graphql::parse_graphql_operations(ops, "o.graphql").unwrap();
    for op in ops {
        schema.add_operation(op);
    }
    for frag in frags {
        schema.add_fragment(frag);
    }

    let output = generate::generate(&schema, &[Target::Sdk], None);

    assert!(output.contains_key("client.ts"));
    assert!(!output.contains_key("hooks.ts"));
    assert!(!output.contains_key("mocks.ts"));
}

#[test]
fn test_empty_operations_no_output() {
    let schema = odk_schema::CompiledSchema::new("Empty");
    let output = generate::generate(
        &schema,
        &[Target::Sdk, Target::ReactQuery, Target::Msw],
        None,
    );
    assert!(output.is_empty());
}

#[test]
fn test_mcp_generate_pipeline() {
    let sdl = r#"
        type Query {
            posts(first: Int, condition: PostCondition): PostsConnection
            projects: ProjectsConnection
        }

        type Mutation {
            createPost(input: CreatePostInput!): CreatePostPayload
            createVote(input: CreateVoteInput!): CreateVotePayload
        }

        type PostsConnection { totalCount: Int! }
        type ProjectsConnection { totalCount: Int! }
        type CreatePostPayload { post: Post }
        type CreateVotePayload { vote: Vote }
        type Post { rowId: ID! title: String }
        type Vote { rowId: ID! voteType: String! }
        input PostCondition { projectId: ID }
        input CreatePostInput { post: PostInput! }
        input CreateVoteInput { vote: VoteInput! }
        input PostInput { title: String projectId: ID! }
        input VoteInput { postId: ID! voteType: String! }
    "#;

    let ops_doc = r#"
        query ListPosts($first: Int, $condition: PostCondition) {
            posts(first: $first, condition: $condition) {
                totalCount
            }
        }

        mutation SubmitPost($input: CreatePostInput!) {
            createPost(input: $input) {
                post {
                    rowId
                    title
                }
            }
        }

        mutation CastVote($input: CreateVoteInput!) {
            createVote(input: $input) {
                vote {
                    rowId
                    voteType
                }
            }
        }
    "#;

    let mut schema = graphql::parse_graphql_sdl(sdl, "schema.graphql").unwrap();
    let (ops, frags) = graphql::parse_graphql_operations(ops_doc, "ops.graphql").unwrap();
    for op in ops {
        schema.add_operation(op);
    }
    for frag in frags {
        schema.add_fragment(frag);
    }

    let gen_cfg = GenerateConfig {
        targets: vec!["mcp".into()],
        output: None,
        operations: None,
        mcp: Some(McpGenerateConfig {
            name: "backfeed-mcp".into(),
            description: Some("Backfeed feedback tools".into()),
            port: 4001,
            path: "/mcp".into(),
            tools: vec![
                McpToolConfig {
                    operation: "SubmitPost".into(),
                    name: "submit_feedback".into(),
                    description: "Submit feedback to a board".into(),
                    parameters: vec![],
                },
                McpToolConfig {
                    operation: "ListPosts".into(),
                    name: "list_feedback".into(),
                    description: "List feedback posts".into(),
                    parameters: vec![],
                },
            ],
            forward_headers: vec![],
        }),
    };

    let output = generate::generate(&schema, &[Target::Mcp], Some(&gen_cfg));

    assert!(output.contains_key("server.ts"), "Missing server.ts");
    assert!(output.contains_key("package.json"), "Missing package.json");

    let server = &output["server.ts"];

    // MCP SDK + HTTP transport imports
    assert!(server.contains("import { McpServer }"));
    assert!(
        server.contains("WebStandardStreamableHTTPServerTransport"),
        "Should use HTTP transport, not STDIO"
    );
    assert!(
        !server.contains("StdioServerTransport"),
        "Should not embed STDIO transport"
    );

    // GraphQL client wired to GRAPHQL_ENDPOINT env var
    assert!(server.contains("GraphQLClient"));
    assert!(server.contains("process.env.GRAPHQL_ENDPOINT"));

    // Bun.serve hosts the transport at the configured path/port
    assert!(server.contains("Bun.serve"));
    assert!(server.contains("process.env.MCP_PORT ?? 4001"));
    assert!(server.contains("url.pathname === \"/mcp\""));

    // Per-request server lifecycle is wrapped in registerTools()
    assert!(server.contains("function registerTools(server: McpServer)"));
    assert!(server.contains("registerTools(server)"));
    assert!(
        server.contains("enableJsonResponse: true"),
        "Stateless transport must enable JSON response mode"
    );

    // Tool registrations come from the curated config
    assert!(server.contains("\"submit_feedback\""));
    assert!(server.contains("\"list_feedback\""));
    assert!(server.contains("Submit feedback to a board"));

    // GraphQL operation documents are embedded
    assert!(server.contains("const SubmitPostDocument = `"));
    assert!(server.contains("const ListPostsDocument = `"));
    assert!(server.contains("mutation SubmitPost"));
    assert!(server.contains("query ListPosts"));

    // Operations not in the tools list should not be exposed
    assert!(
        !server.contains("CastVote"),
        "CastVote should be filtered out"
    );
    assert!(!server.contains("createVote"));

    // package.json wires the right runtime deps
    let pkg = &output["package.json"];
    assert!(pkg.contains("\"name\": \"backfeed-mcp\""));
    assert!(pkg.contains("@modelcontextprotocol/sdk"));
    assert!(pkg.contains("graphql-request"));
    assert!(pkg.contains("zod"));
}

#[test]
fn test_mcp_curated_parameters_pipeline() {
    let sdl = r#"
        type Query {
            posts(condition: PostCondition): PostsConnection
        }

        type PostsConnection { totalCount: Int! }
        input PostCondition { projectId: ID }
    "#;

    let ops_doc = r#"
        query ListPosts($condition: PostCondition) {
            posts(condition: $condition) {
                totalCount
            }
        }
    "#;

    let mut schema = graphql::parse_graphql_sdl(sdl, "schema.graphql").unwrap();
    let (ops, frags) = graphql::parse_graphql_operations(ops_doc, "ops.graphql").unwrap();
    for op in ops {
        schema.add_operation(op);
    }
    for frag in frags {
        schema.add_fragment(frag);
    }

    let gen_cfg = GenerateConfig {
        targets: vec!["mcp".into()],
        output: None,
        operations: None,
        mcp: Some(McpGenerateConfig {
            name: "curated-mcp".into(),
            description: None,
            port: 4001,
            path: "/mcp".into(),
            tools: vec![McpToolConfig {
                operation: "ListPosts".into(),
                name: "list_posts".into(),
                description: "List posts in a project".into(),
                parameters: vec![McpToolParameter {
                    name: "projectId".into(),
                    type_: "string".into(),
                    format: Some("uuid".into()),
                    enum_values: vec![],
                    required: true,
                    default: None,
                    description: Some("Project to filter by".into()),
                    map_to: "condition.projectId".into(),
                }],
            }],
            forward_headers: vec![],
        }),
    };

    let output = generate::generate(&schema, &[Target::Mcp], Some(&gen_cfg));

    let server = &output["server.ts"];

    // Curated parameters trigger the set() helper for nested variable assembly
    assert!(
        server.contains("function set("),
        "Curated parameters require the set() helper"
    );

    // Curated zod schema with format refinement, not the variable-derived schema
    assert!(server.contains("z.string().uuid()"));
    assert!(server.contains("Project to filter by"));

    // Variable assembly via dot-path mapping
    assert!(server.contains("set(variables, \"condition.projectId\", params.projectId)"));
    assert!(server.contains("graphql.request(ListPostsDocument, variables)"));
}
