---
"@omnidotdev/odk": patch
---

Fix `odk.emit` and `odk.toTypeSpec` fidelity for Zod 4 schemas.

- `odk.emit` now works when ODK is installed as a dependency: the TypeSpec emitters are resolved from the consumer's `node_modules` instead of a path that only exists inside the ODK repo, which failed with `Couldn't resolve import "@typespec/json-schema"`.
- Integers (`z.number().int()`, `z.int()`) emit as `type: integer` (TypeSpec `safeint`, or `int32` when explicit bounds fit 32 bits) instead of `int64`, which JSON Schema renders as a string, and no longer carry synthetic safe-integer `minimum`/`maximum` bounds.
- `.describe()` text is emitted as `@doc`, so it reaches JSON Schema and OpenAPI `description` on models, properties, and enums.
- `.default()` fields are optional and carry their default (`field?: T = value`).
- `.nullable()` emits `T | null` (JSON Schema `null`, OpenAPI `nullable: true`) instead of `T | unknown`.
- Array `.min()`/`.max()` emit `minItems`/`maxItems`.
- The generated namespace is a `@service`, so `@typespec/openapi3` includes every model as a component schema, and `odk.raw` blocks and `odk.addImport` imports are now emitted on the TypeScript path.

The compiled schema IR gains a `safeint` scalar (understood by the Rust CLI and its generators) and optional `nullable`, `default`, `minItems`, and `maxItems` field properties.
