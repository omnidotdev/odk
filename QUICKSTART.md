# ODK Quickstart

Define your schema once, generate OpenAPI, Protobuf, Drizzle ORM, and more.

## Option A: YAML + CLI (simplest)

Create `schema.odk.yaml`:

```yaml
version: "1.0"
name: MyAPI

models:
  User:
    fields:
      id:
        type: string
        format: uuid
      email:
        type: string
        format: email
      name:
        type: string
        optional: true
```

Compile (defaults to OpenAPI + JSON Schema):

```sh
odk compile schema.odk.yaml
```

Specify emitters:

```sh
odk compile schema.odk.yaml -e @typespec/openapi3,@typespec/protobuf -o generated/
```

## Option B: TypeScript with Zod

Create `schema.ts`:

```typescript
import { odk } from "@omnidotdev/odk";
import { z } from "zod";

const User = z.object({
  id: z.string().uuid(),
  email: z.string().email(),
  name: z.string().optional(),
});

const schema = odk.schema("MyAPI", { User });

await odk.emit(schema, {
  targets: ["@typespec/openapi3"],
  outDir: "./generated",
});
```

Run directly with bun:

```sh
bun run schema.ts
```

## Option C: Python

Create `schema.py`:

```python
from odk_pdk import Schema, Model, Field

schema = Schema("MyAPI")

User = Model("User", fields=[
    Field("id", type="string", format="uuid"),
    Field("email", type="string", format="email"),
    Field("name", type="string", optional=True),
])

schema.add_model(User)
print(schema.to_json())
```

Compile via CLI:

```sh
python schema.py > schema.json
odk compile schema.json
```

## Installation

```sh
# CLI (Rust)
cargo install odk-cli

# TypeScript SDK (optional)
bun add @omnidotdev/odk zod

# Python PDK (optional)
pip install odk-pdk
```

## Available Emitters

| Emitter                        | Output             | Flag                              |
| ------------------------------ | ------------------ | --------------------------------- |
| `@typespec/openapi3`           | OpenAPI 3.x spec   | `-e @typespec/openapi3`           |
| `@typespec/json-schema`        | JSON Schema        | `-e @typespec/json-schema`        |
| `@typespec/protobuf`           | Protocol Buffers   | `-e @typespec/protobuf`           |
| `@omnidotdev/typespec-drizzle` | Drizzle ORM schema | `-e @omnidotdev/typespec-drizzle` |

Default (no `-e` flag): `@typespec/json-schema` + `@typespec/openapi3`

## CLI Reference

```sh
odk compile <input> [options]

Options:
  -o, --out <dir>      Output directory (default: ./generated)
  -e, --emit <list>    TypeSpec emitters (comma-separated)
  --keep-temp          Keep temporary TypeSpec files

odk check <input>      Validate schema without emitting
odk typespec <input>   Generate TypeSpec source only
odk init               Create a new project
```

## Next Steps

- See `examples/` for complete schemas
- Read `docs/` for detailed guides

CLI is good, but odk should support config files (json, jsonc, yaml, ts) with option to pass config file as a path. look up best practices for this
