//! Blog API Schema Plugin
//!
//! This example demonstrates how to define schemas in Rust
//! and compile them to a WASM plugin for use with the ODK CLI.

use odk_pdk::{datetime_field, email_field, string_enum, uuid_field};
use odk_pdk::{prelude::*, string_field};

/// Build the Blog API schema
fn build_schema() -> CompiledSchema {
    let mut schema = CompiledSchema::new("BlogAPI");

    let mut comment = CompiledModel::new("Comment");
    comment.add_field(uuid_field("id"));
    comment.add_field(string_field("author"));
    comment.add_field(string_field("content"));
    comment.add_field(datetime_field("created_at"));
    schema.add_model(comment);

    // UserRole enum
    schema.add_union(string_enum("UserRole", &["admin", "author", "reader"]));

    // User model
    let mut user = CompiledModel::new("User");
    user.add_field(uuid_field("id"));
    user.add_field(email_field("email"));
    user.add_field(
        CompiledField::new("name", CompiledType::string()).with_constraints(FieldConstraints {
            min_length: Some(1),
            max_length: Some(100),
            ..Default::default()
        }),
    );
    user.add_field(CompiledField::new(
        "role",
        CompiledType::reference("UserRole"),
    ));
    user.add_field(datetime_field("createdAt"));
    user.add_field(
        CompiledField::new("bio", CompiledType::string())
            .optional()
            .with_constraints(FieldConstraints {
                max_length: Some(500),
                ..Default::default()
            }),
    );
    schema.add_model(user);

    // Post model
    let mut post = CompiledModel::new("Post");
    post.add_field(uuid_field("id"));
    post.add_field(
        CompiledField::new("title", CompiledType::string()).with_constraints(FieldConstraints {
            min_length: Some(1),
            max_length: Some(200),
            ..Default::default()
        }),
    );
    post.add_field(
        CompiledField::new("slug", CompiledType::string()).with_constraints(FieldConstraints {
            pattern: Some("^[a-z0-9-]+$".to_string()),
            ..Default::default()
        }),
    );
    post.add_field(CompiledField::new("content", CompiledType::string()));
    post.add_field(uuid_field("authorId"));
    post.add_field(CompiledField::new("published", CompiledType::boolean()));
    post.add_field(CompiledField::new(
        "tags",
        CompiledType::array(CompiledType::string()),
    ));
    post.add_field(datetime_field("createdAt"));
    post.add_field(datetime_field("updatedAt").optional());
    schema.add_model(post);

    // Comment model
    let mut comment = CompiledModel::new("Comment");
    comment.add_field(uuid_field("id"));
    comment.add_field(uuid_field("postId"));
    comment.add_field(uuid_field("authorId"));
    comment.add_field(
        CompiledField::new("content", CompiledType::string()).with_constraints(FieldConstraints {
            min_length: Some(1),
            max_length: Some(1000),
            ..Default::default()
        }),
    );
    comment.add_field(datetime_field("createdAt"));
    schema.add_model(comment);

    schema
}

// Export the get_schema function for the Extism host
odk_pdk::odk_export_schema!(build_schema);
