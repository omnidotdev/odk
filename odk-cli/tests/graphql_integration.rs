//! Integration test: GraphQL SDL + operations -> CompiledSchema

use odk_schema::OperationKind;

#[test]
fn test_full_graphql_pipeline() {
    use odk_cli::loaders::graphql;

    // Parse SDL
    let sdl = r#"
        type Query {
            user(id: ID!): User
            users(limit: Int): [User!]!
        }

        type Mutation {
            createUser(input: CreateUserInput!): User!
        }

        type User {
            id: ID!
            name: String!
            email: String
            posts: [Post!]!
        }

        type Post {
            id: ID!
            title: String!
            author: User!
        }

        input CreateUserInput {
            name: String!
            email: String!
        }

        enum Role {
            ADMIN
            USER
            MODERATOR
        }

        union SearchResult = User | Post
    "#;

    let mut schema = graphql::parse_graphql_sdl(sdl, "schema.graphql").unwrap();

    // Parse operations
    let ops_doc = r#"
        fragment UserFields on User {
            id
            name
            email
        }

        query GetUser($id: ID!) {
            user(id: $id) {
                ...UserFields
                posts {
                    id
                    title
                }
            }
        }

        mutation CreateUser($input: CreateUserInput!) {
            createUser(input: $input) {
                ...UserFields
            }
        }
    "#;

    let (ops, frags) =
        graphql::parse_graphql_operations(ops_doc, "operations.graphql").unwrap();

    for op in ops {
        schema.add_operation(op);
    }
    for frag in frags {
        schema.add_fragment(frag);
    }

    // Verify types
    assert_eq!(schema.models.len(), 5); // Query, Mutation, User, Post, CreateUserInput
    assert_eq!(schema.unions.len(), 2); // Role, SearchResult

    // Verify operations
    assert_eq!(schema.operations.len(), 2);
    assert!(matches!(schema.operations[0].kind, OperationKind::Query));
    assert!(matches!(schema.operations[1].kind, OperationKind::Mutation));

    // Verify fragments
    assert_eq!(schema.fragments.len(), 1);
    assert_eq!(schema.fragments[0].name, "UserFields");

    // Verify round-trip through JSON
    let json = schema.to_json().unwrap();
    let parsed = odk_schema::CompiledSchema::from_json(&json).unwrap();
    assert_eq!(parsed.models.len(), schema.models.len());
    assert_eq!(parsed.unions.len(), schema.unions.len());
    assert_eq!(parsed.operations.len(), schema.operations.len());
    assert_eq!(parsed.fragments.len(), schema.fragments.len());
}
