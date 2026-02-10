"""Tests for ODK PDK Python."""

import json
import sys
from pathlib import Path

# Add src to path for testing without install
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from odk_pdk import Field, Model, Schema, Union


def test_simple_schema():
    """Test creating a simple schema."""
    schema = Schema("TestAPI")

    User = Model(
        "User",
        fields=[
            Field("id", type="string", format="uuid"),
            Field("email", type="string", format="email"),
            Field("name", type="string", optional=True),
        ],
    )

    schema.add_model(User)

    # Check dict structure
    data = schema.to_dict()
    assert data["name"] == "TestAPI"
    assert len(data["models"]) == 1
    assert data["models"][0]["name"] == "User"
    assert len(data["models"][0]["fields"]) == 3


def test_field_constraints():
    """Test field constraints are serialized correctly."""
    field = Field(
        "title",
        type="string",
        min_length=1,
        max_length=200,
        pattern="^[a-z]+$",
    )

    data = field.to_dict()
    assert data["constraints"]["minLength"] == 1
    assert data["constraints"]["maxLength"] == 200
    assert data["constraints"]["pattern"] == "^[a-z]+$"


def test_field_decorators():
    """Test field decorators are in extensions."""
    field = Field(
        "id",
        type="string",
        format="uuid",
        decorators=["@id", "@key"],
    )

    data = field.to_dict()
    assert "extensions" in data
    assert len(data["extensions"]["known"]) == 2
    assert data["extensions"]["known"][0]["kind"] == "decorator"
    assert data["extensions"]["known"][0]["value"] == "@id"


def test_union_string_enum():
    """Test string enum union."""
    role = Union(
        "UserRole",
        variants=["admin", "author", "reader"],
        description="User permission levels",
    )

    data = role.to_dict()
    assert data["name"] == "UserRole"
    assert len(data["variants"]) == 3
    assert data["variants"][0]["name"] == "admin"
    assert data["variants"][0]["type"]["kind"] == "literal"
    assert data["variants"][0]["type"]["value"] == "admin"


def test_model_decorators():
    """Test model decorators."""
    model = Model(
        "User",
        decorators=["@table"],
        fields=[Field("id", type="string")],
    )

    data = model.to_dict()
    assert "extensions" in data
    assert data["extensions"]["known"][0]["value"] == "@table"


def test_schema_with_raw_block():
    """Test adding raw TypeSpec blocks."""
    schema = Schema("TestAPI")
    schema.add_raw(
        """
        @route("/api")
        interface Api {
            @get list(): void;
        }
    """,
        description="API routes",
    )

    data = schema.to_dict()
    assert len(data["raw_blocks"]) == 1
    assert "@route" in data["raw_blocks"][0]["content"]


def test_schema_imports():
    """Test schema imports."""
    schema = Schema("TestAPI")
    schema.add_import("@typespec/http")
    schema.add_import("@omnidotdev/typespec-drizzle")

    data = schema.to_dict()
    assert "@typespec/http" in data["imports"]
    assert "@omnidotdev/typespec-drizzle" in data["imports"]


def test_field_ref():
    """Test field with type reference."""
    field = Field("status", ref="PostStatus")

    data = field.to_dict()
    assert data["type"]["kind"] == "ref"
    assert data["type"]["name"] == "PostStatus"


def test_field_array():
    """Test array field."""
    field = Field("tags", type="string", array=True)

    data = field.to_dict()
    assert data["type"]["kind"] == "array"
    assert data["type"]["items"]["kind"] == "scalar"
    assert data["type"]["items"]["name"] == "string"


def test_to_json():
    """Test JSON serialization."""
    schema = Schema("TestAPI")
    schema.add_model(Model("User", fields=[Field("id", type="string")]))

    json_str = schema.to_json()
    data = json.loads(json_str)

    assert data["name"] == "TestAPI"
    assert len(data["models"]) == 1


def test_full_blog_schema():
    """Test a realistic blog schema."""
    schema = Schema("BlogAPI")
    schema.add_import("@typespec/http")

    # Add enums
    UserRole = Union(
        "UserRole",
        variants=["admin", "author", "reader"],
    )
    PostStatus = Union(
        "PostStatus",
        variants=["draft", "published", "archived"],
    )
    schema.add_unions([UserRole, PostStatus])

    # Add models
    User = Model(
        "User",
        decorators=["@table"],
        fields=[
            Field("id", type="string", format="uuid", decorators=["@id"]),
            Field("email", type="string", format="email"),
            Field("role", ref="UserRole"),
        ],
    )

    Post = Model(
        "Post",
        decorators=["@table"],
        fields=[
            Field("id", type="string", format="uuid", decorators=["@id"]),
            Field("title", type="string", min_length=1, max_length=200),
            Field("authorId", type="string", format="uuid"),
            Field("status", ref="PostStatus"),
        ],
    )

    schema.add_models([User, Post])

    # Verify structure
    data = schema.to_dict()
    assert data["name"] == "BlogAPI"
    assert len(data["unions"]) == 2
    assert len(data["models"]) == 2
    assert "@typespec/http" in data["imports"]

    # Verify JSON is valid
    json_str = schema.to_json()
    parsed = json.loads(json_str)
    assert parsed["name"] == "BlogAPI"


if __name__ == "__main__":
    # Run tests manually
    test_simple_schema()
    test_field_constraints()
    test_field_decorators()
    test_union_string_enum()
    test_model_decorators()
    test_schema_with_raw_block()
    test_schema_imports()
    test_field_ref()
    test_field_array()
    test_to_json()
    test_full_blog_schema()
    print("All tests passed!")
