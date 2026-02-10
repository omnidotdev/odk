# @omnidotdev/odk

Define schemas with Zod, ArkType, or Valibot. Emit to OpenAPI, Protobuf, JSON Schema, and more via TypeSpec.

## Installation

```bash
bun add @omnidotdev/odk

# Plus your schema library of choice:
bun add zod
# or: bun add arktype
# or: bun add valibot @valibot/to-json-schema
```

## Quick Start

```typescript
import { odk } from "@omnidotdev/odk";
import { z } from "zod";

// Define schemas with your favorite library
const UserRole = z.enum(["admin", "user", "guest"]);

const User = z.object({
  id: z.string().uuid(),
  email: z.string().email(),
  name: z.string().min(1).max(100),
  role: UserRole,
  createdAt: z.iso.datetime(),
});

// Create ODK schema
const schema = odk.schema("MyAPI", { UserRole, User });

// Generate TypeSpec source
const tsp = odk.toTypeSpec(schema);
console.log(tsp);

// Or emit to JSON Schema, OpenAPI, etc.
await odk.emit(schema, {
  targets: ["@typespec/json-schema"],
  outDir: "./generated",
});
```

## API

### `odk.schema(name, types)`

Create an ODK schema from Standard Schema-compliant types.

- `name`: Namespace name for the generated TypeSpec
- `types`: Record of named types (Zod, ArkType, or Valibot schemas)

### `odk.toTypeSpec(schema, namespace?)`

Generate TypeSpec source code from an ODK schema.

### `odk.emit(schema, options)`

Emit to target formats using the TypeSpec compiler.

Options:
- `targets`: Array of emitters (e.g., `["@typespec/json-schema", "@typespec/openapi3"]`)
- `outDir`: Output directory

### `odk.toJSON(schema)`

Serialize the compiled schema to JSON (for use with the Rust CLI).

## Supported Schema Libraries

| Library | Version | Support |
|---------|---------|---------|
| Zod | 4.x | Native `toJSONSchema()` |
| Zod | 3.x | Via `zod-to-json-schema` |
| ArkType | 2.x | Native `toJsonSchema()` |
| Valibot | 1.x | Via `@valibot/to-json-schema` |

## TypeSpec Emitters

ODK supports any TypeSpec emitter. Common ones:

- `@typespec/json-schema` - JSON Schema output
- `@typespec/openapi3` - OpenAPI 3.0 (for HTTP APIs)
- `@typespec/protobuf` - Protocol Buffers
- Community emitters (install separately)

## Constraint Mapping

| Schema Constraint | TypeSpec Decorator |
|-------------------|-------------------|
| `.min(n)` / `.minLength(n)` | `@minLength(n)` |
| `.max(n)` / `.maxLength(n)` | `@maxLength(n)` |
| `.min(n)` (numbers) | `@minValue(n)` |
| `.max(n)` (numbers) | `@maxValue(n)` |
| `.regex(pattern)` | `@pattern("...")` |
| `.email()` | `@format("email")` |
| `.uuid()` | `@format("uuid")` |
| `.datetime()` | `@format("date-time")` |

### Non-Serializable Constraints

Some constraints can't be represented in TypeSpec/JSON Schema:

- `.transform()` - Runtime only
- `.refine()` / `.superRefine()` - Runtime only
- `.preprocess()` - Runtime only

These will emit warnings but won't break compilation.

## Examples

See the `examples/` directory:

- `blog-zod.ts` - Zod 4 example
- `blog-arktype.ts` - ArkType example
- `emit-test.ts` - End-to-end emit test

## License

The code in this repository is licensed under MIT, &copy; [Omni LLC](https://omni.dev). See [LICENSE.md](LICENSE.md) for more information.
