# @omnidotdev/odk

## 0.1.3

### Patch Changes

- [#17](https://github.com/omnidotdev/odk/pull/17) [`f3f4f28`](https://github.com/omnidotdev/odk/commit/f3f4f2899981d6996d0c7189c64a1f94e9859078) Thanks [@coopbri](https://github.com/coopbri)! - Reject unsupported tagged-union output up front instead of emitting silently wrong code.

  Data-carrying (tagged) unions are emitted only by the `rust` and `typescript` generate targets. Previously the other paths degraded silently: `odk generate -t python` collapsed a tagged union to a bare string enum, dropping every variant payload, and `odk compile` emitted an undiscriminated union alias, dropping the discriminator from the OpenAPI / JSON Schema output. The `rust` target also boxes only a direct self-reference, so a reference cycle through other types (mutual recursion) produced infinitely-sized Rust that failed at the consumer's `cargo build`.

  Each of these now fails with a clear, actionable error that names the union (or the recursion cycle) and points to `-t rust,typescript`, rather than producing output that looks valid but is wrong. The supported paths are unchanged.

- [#19](https://github.com/omnidotdev/odk/pull/19) [`3a9d77f`](https://github.com/omnidotdev/odk/commit/3a9d77fa804e5f7f0a5f4a988368599d6f9c6ed3) Thanks [@coopbri](https://github.com/coopbri)! - Fix `odk.emit` and `odk.toTypeSpec` fidelity for Zod 4 schemas.

  - `odk.emit` now works when ODK is installed as a dependency: the TypeSpec emitters are resolved from the consumer's `node_modules` instead of a path that only exists inside the ODK repo, which failed with `Couldn't resolve import "@typespec/json-schema"`.
  - Integers (`z.number().int()`, `z.int()`) emit as `type: integer` (TypeSpec `safeint`, or `int32` when explicit bounds fit 32 bits) instead of `int64`, which JSON Schema renders as a string, and no longer carry synthetic safe-integer `minimum`/`maximum` bounds.
  - `.describe()` text is emitted as `@doc`, so it reaches JSON Schema and OpenAPI `description` on models, properties, and enums.
  - `.default()` fields are optional and carry their default (`field?: T = value`).
  - `.nullable()` emits `T | null` (JSON Schema `null`, OpenAPI `nullable: true`) instead of `T | unknown`.
  - Array `.min()`/`.max()` emit `minItems`/`maxItems`.
  - The generated namespace is a `@service`, so `@typespec/openapi3` includes every model as a component schema, and `odk.raw` blocks and `odk.addImport` imports are now emitted on the TypeScript path.

  The compiled schema IR gains a `safeint` scalar (understood by the Rust CLI and its generators) and optional `nullable`, `default`, `minItems`, and `maxItems` field properties.

## 0.1.2

### Patch Changes

- [#14](https://github.com/omnidotdev/odk/pull/14) [`7aa0a7b`](https://github.com/omnidotdev/odk/commit/7aa0a7b30fbd12f1a01249e20a91e167b827e6c5) Thanks [@coopbri](https://github.com/coopbri)! - Security and dependency hardening for the Rust toolchain:

  - Upgrade the WASM plugin runtime (extism 1.20 to 1.41, pulling wasmtime 37 to 49.0.5), clearing nine RUSTSEC advisories including sandbox escapes and Cranelift miscompilations.
  - Bump the `zstd` family and `crossbeam-epoch` to patched versions.
  - Replace the unmaintained `serde_yaml` with the maintained `serde_norway` fork (identical API).

  No user-facing behavior change; all tests, examples, and the WASM plugin host validated. CI now runs `cargo audit`.

## 0.1.1

### Patch Changes

- [#11](https://github.com/omnidotdev/odk/pull/11) [`e493a66`](https://github.com/omnidotdev/odk/commit/e493a66a9ccda7fdb1dc560be6e3ca752df3285c) Thanks [@coopbri](https://github.com/coopbri)! - Publish the ODK Python PDK (`odk-pdk`) to the Omni Manifold registry via trusted publishing, installable with `pip install odk-pdk --index-url https://api.manifold.omni.dev/pypi/omnidotdev/simple/`. The TypeScript SDK and the Rust crates are unchanged in this release and move to the same version.

## 0.1.0

### Minor Changes

- [`475efd7`](https://github.com/omnidotdev/odk/commit/475efd7b67b8457b375e78db26a10eb00097a18b) Thanks [@coopbri](https://github.com/coopbri)! - Initial release. ODK models a schema once and emits to every target: OpenAPI, JSON Schema, Protobuf, and Drizzle via `compile`, plus typed SDKs, React Query hooks, MSW mocks, an MCP server, and TypeScript, Rust, and Python models via `generate`. Schemas are authored in TypeSpec, YAML, JSON, or TypeScript, with WASM plugin support through Extism and language PDKs. This release carries the recursive data-carrying discriminated union grammar and fixed-size arrays end to end across the TypeScript, Rust, and Python emitters.
