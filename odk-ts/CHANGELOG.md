# @omnidotdev/odk

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
