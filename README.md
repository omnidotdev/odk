# Omni Development Kit (ODK)

Define your data model once. Generate OpenAPI, Protobuf, Drizzle ORM, JSON Schema, and more.

## Quickstart

**See [QUICKSTART.md](QUICKSTART.md) for a 2-minute getting started guide.**

## How It Works

```
Your Schema (YAML, TypeScript, or Rust)
            ↓
     CompiledSchema IR
            ↓
        TypeSpec
            ↓
   Any TypeSpec Emitter
   (OpenAPI, Protobuf, Drizzle, etc.)
```

## Installation

```sh
# TypeScript SDK
bun add @omnidotdev/odk zod

# CLI
cargo install odk-cli
```

## Example

**YAML:**

```yaml
version: "1.0"
name: MyAPI
models:
  User:
    fields:
      id: { type: string, format: uuid }
      email: { type: string, format: email }
```

```sh
odk compile schema.odk.yaml -o generated/
```

**TypeScript:**

```typescript
import { odk } from "@omnidotdev/odk";
import { z } from "zod";

const User = z.object({
  id: z.string().uuid(),
  email: z.string().email(),
});

const schema = odk.schema("MyAPI", { User });
await odk.emit(schema, {
  targets: ["@typespec/openapi3"],
  outDir: "./generated",
});
```

## Documentation

Full documentation is hosted at [docs.omni.dev/products/odk](https://docs.omni.dev/products/odk).

| Guide                                                                              | Description                 |
| ---------------------------------------------------------------------------------- | --------------------------- |
| [QUICKSTART.md](QUICKSTART.md)                                                     | Get started in 2 minutes    |
| [YAML format](https://docs.omni.dev/products/odk/yaml-format)                      | YAML schema reference       |
| [TypeScript SDK](https://docs.omni.dev/products/odk/typescript-sdk)                | TypeScript SDK API          |
| [Python PDK](https://docs.omni.dev/products/odk/python-pdk)                        | Python PDK                  |
| [Rust plugins](https://docs.omni.dev/products/odk/rust-plugins)                    | Rust WASM plugins           |
| [Extensions](https://docs.omni.dev/products/odk/extensions)                        | Escape hatches & extensions |

## Project Structure

```
odk/
├── odk-schema/     # Rust: IR types
├── odk-cli/        # Rust: CLI
├── odk-ts/         # TypeScript SDK
├── odk-pdk-rust/   # Rust plugin SDK
├── odk-pdk-py/     # Python PDK
└── examples/       # Example schemas
```

## License

The code in this repository is licensed under Apache 2.0, &copy; [Omni LLC](https://omni.dev). See [LICENSE.md](LICENSE.md) for more information.
