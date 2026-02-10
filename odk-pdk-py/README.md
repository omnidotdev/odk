# ODK Python PDK

Define schemas in Python, generate OpenAPI, JSON Schema, TypeScript, and more.

## Installation

```sh
pip install odk-pdk
```

For local development:

```sh
cd odk-pdk-py
pip install -e .
```

## Quick Start

```python
from odk_pdk import Schema, Model, Field, Union

# Create schema
schema = Schema("MyAPI")

# Define a model
User = Model(
    "User",
    fields=[
        Field("id", type="string", format="uuid"),
        Field("email", type="string", format="email"),
        Field("name", type="string", optional=True),
    ],
)

schema.add_model(User)

# Output JSON for ODK CLI
print(schema.to_json())
```

## Usage with ODK CLI

```sh
# Generate schema JSON
python schema.py > schema.json

# Compile with native emitters (no Node.js required)
odk compile schema.json --native

# Compile with TypeSpec emitters
odk compile schema.json -e @typespec/openapi3,@typespec/json-schema
```

## API Reference

See the full documentation at [docs/python-pdk.md](../docs/python-pdk.md).

### Core Classes

- **Schema** - Root container for your API definition
- **Model** - Defines a structured type with fields
- **Field** - Defines a field within a model
- **Union** - Defines an enum or discriminated union

### Scalar Types

| Type             | Description                  |
| ---------------- | ---------------------------- |
| `string`         | UTF-8 string                 |
| `int32`          | 32-bit signed integer        |
| `int64`          | 64-bit signed integer        |
| `float32`        | 32-bit float                 |
| `float64`        | 64-bit float                 |
| `boolean`        | Boolean                      |
| `date`           | Date without time            |
| `datetime`       | Datetime                     |
| `utcDateTime`    | UTC datetime (TypeSpec)      |
| `offsetDateTime` | Datetime with timezone       |
| `plainDate`      | Date without time (TypeSpec) |
| `plainTime`      | Time without date            |
| `duration`       | ISO 8601 duration            |
| `bytes`          | Binary data                  |
| `url`            | URL string                   |

## Example

See [examples/blog.odk.py](../examples/blog.odk.py) for a complete example.

```python
from odk_pdk import Schema, Model, Field, Union

schema = Schema("BlogAPI")
schema.add_import("@typespec/http")

# Enums
UserRole = Union("UserRole", variants=["admin", "author", "reader"])
PostStatus = Union("PostStatus", variants=["draft", "published", "archived"])

# Models
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
        Field("authorId", type="string", decorators=["@manyToOne(User)"]),
        Field("status", ref="PostStatus"),
    ],
)

schema.add_unions([UserRole, PostStatus])
schema.add_models([User, Post])

print(schema.to_json())
```

## License

The code in this repository is licensed under MIT, &copy; [Omni LLC](https://omni.dev). See [LICENSE.md](LICENSE.md) for more information.
