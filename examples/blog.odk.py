"""
Blog API Schema - Python Format

Define a blog API schema using the ODK Python PDK.

Usage:
  # Test locally (outputs JSON)
  python blog.odk.py

  # When WASM support is available:
  # componentize-py blog.odk.py -o blog.wasm
  # odk compile blog.wasm -o generated/
"""

import sys
from pathlib import Path

# Add odk-pdk-py to path for local development
sys.path.insert(0, str(Path(__file__).parent.parent / "odk-pdk-py" / "src"))

from odk_pdk import Field, Model, Schema, Union

# Create the schema
schema = Schema("BlogAPI")

# Add imports
schema.add_import("@typespec/http")
schema.add_import("@omnidotdev/typespec-drizzle")

# Define unions (enums)
UserRole = Union(
    "UserRole",
    variants=["admin", "author", "reader"],
    description="User permission levels",
)

PostStatus = Union(
    "PostStatus",
    variants=["draft", "published", "archived"],
    description="Publication status of a post",
)

# Define models
User = Model(
    "User",
    description="A blog user account",
    decorators=["@table"],
    fields=[
        Field(
            "id",
            type="string",
            format="uuid",
            decorators=["@id"],
            description="Unique user identifier",
        ),
        Field(
            "email",
            type="string",
            format="email",
            description="User's email address",
        ),
        Field(
            "username",
            type="string",
            min_length=3,
            max_length=32,
            pattern="^[a-zA-Z0-9_]+$",
            description="Unique username",
        ),
        Field(
            "displayName",
            type="string",
            optional=True,
            max_length=100,
            description="Display name shown on posts",
        ),
        Field(
            "role",
            ref="UserRole",
            description="User's permission level",
        ),
        Field("createdAt", type="utcDateTime", decorators=["@createdAt"]),
        Field("updatedAt", type="utcDateTime", decorators=["@updatedAt"]),
    ],
)

Post = Model(
    "Post",
    description="A blog post",
    decorators=["@table"],
    fields=[
        Field("id", type="string", format="uuid", decorators=["@id"]),
        Field(
            "title",
            type="string",
            min_length=1,
            max_length=200,
            description="Post title",
        ),
        Field(
            "slug",
            type="string",
            pattern="^[a-z0-9-]+$",
            description="URL-friendly slug",
        ),
        Field("content", type="string", description="Post content in Markdown"),
        Field(
            "excerpt",
            type="string",
            optional=True,
            max_length=500,
            description="Short excerpt for previews",
        ),
        Field(
            "authorId",
            type="string",
            format="uuid",
            decorators=["@manyToOne(User)"],
        ),
        Field("status", ref="PostStatus"),
        Field("publishedAt", type="utcDateTime", optional=True),
        Field("createdAt", type="utcDateTime", decorators=["@createdAt"]),
        Field("updatedAt", type="utcDateTime", decorators=["@updatedAt"]),
    ],
)

Comment = Model(
    "Comment",
    description="A comment on a post",
    decorators=["@table"],
    fields=[
        Field("id", type="string", format="uuid", decorators=["@id"]),
        Field("content", type="string", min_length=1, max_length=2000),
        Field("postId", type="string", format="uuid", decorators=["@manyToOne(Post)"]),
        Field(
            "authorId", type="string", format="uuid", decorators=["@manyToOne(User)"]
        ),
        Field(
            "parentId",
            type="string",
            format="uuid",
            optional=True,
            description="Parent comment ID for nested replies",
        ),
        Field("createdAt", type="utcDateTime", decorators=["@createdAt"]),
    ],
)

Tag = Model(
    "Tag",
    description="A tag for categorizing posts",
    decorators=["@table"],
    fields=[
        Field("id", type="string", format="uuid", decorators=["@id"]),
        Field("name", type="string", min_length=1, max_length=50),
        Field("slug", type="string", pattern="^[a-z0-9-]+$"),
    ],
)

# Add models and unions to schema
schema.add_unions([UserRole, PostStatus])
schema.add_models([User, Post, Comment, Tag])

# Add raw TypeSpec for API routes
schema.add_raw(
    """
@route("/api")
@tag("Blog API")
namespace BlogApiRoutes {
    @route("/posts")
    interface Posts {
        @get list(@query status?: PostStatus): Post[];
        @get read(@path slug: string): Post;
        @post create(@body post: Post): Post;
        @put update(@path id: string, @body post: Post): Post;
        @delete remove(@path id: string): void;
    }

    @route("/users")
    interface Users {
        @get list(): User[];
        @get read(@path id: string): User;
        @post create(@body user: User): User;
    }

    @route("/posts/{postId}/comments")
    interface Comments {
        @get list(@path postId: string): Comment[];
        @post create(@path postId: string, @body comment: Comment): Comment;
    }
}
"""
)


def get_schema() -> str:
    """Entry point for Extism WASM export."""
    return schema.to_json()


if __name__ == "__main__":
    # Print JSON when run directly
    print(get_schema())
