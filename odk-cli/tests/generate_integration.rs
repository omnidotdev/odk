//! Integration test: GraphQL schema + operations → generated client artifacts

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
    use odk_cli::config::{McpGenerateConfig, McpToolConfig};

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

    let mcp_config = McpGenerateConfig {
        name: "backfeed-mcp".into(),
        description: "Backfeed feedback tools".into(),
        tools: vec![
            McpToolConfig {
                operation: "SubmitPost".into(),
                name: Some("submit_feedback".into()),
                description: Some("Submit feedback to a board".into()),
            },
            McpToolConfig {
                operation: "ListPosts".into(),
                name: Some("list_feedback".into()),
                description: None,
            },
        ],
    };

    let output = generate::generate(&schema, &[Target::Mcp], Some(&mcp_config));

    // Should generate index.ts and package.json
    assert!(output.contains_key("index.ts"), "Missing index.ts");
    assert!(output.contains_key("package.json"), "Missing package.json");

    let index = &output["index.ts"];

    // Tool names from config
    assert!(
        index.contains("\"submit_feedback\""),
        "Missing submit_feedback tool"
    );
    assert!(
        index.contains("\"list_feedback\""),
        "Missing list_feedback tool"
    );

    // Description from config
    assert!(index.contains("Submit feedback to a board"));

    // GraphQL documents embedded
    assert!(index.contains("mutation SubmitPost"));
    assert!(index.contains("query ListPosts"));

    // MCP SDK imports
    assert!(index.contains("McpServer"));
    assert!(index.contains("StdioServerTransport"));

    // GraphQL client setup
    assert!(index.contains("GraphQLClient"));
    assert!(index.contains("OMNI_USER_TOKEN"));

    // package.json
    let pkg = &output["package.json"];
    assert!(pkg.contains("@omnidotdev/backfeed-mcp"));
    assert!(pkg.contains("@modelcontextprotocol/sdk"));

    // Should NOT include CastVote (not in config tools list)
    assert!(
        !index.contains("CastVote"),
        "CastVote should not be included"
    );
}
