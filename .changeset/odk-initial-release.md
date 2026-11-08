---
"@omnidotdev/odk": minor
---

Initial release. ODK models a schema once and emits to every target: OpenAPI, JSON Schema, Protobuf, and Drizzle via `compile`, plus typed SDKs, React Query hooks, MSW mocks, an MCP server, and TypeScript, Rust, and Python models via `generate`. Schemas are authored in TypeSpec, YAML, JSON, or TypeScript, with WASM plugin support through Extism and language PDKs. This release carries the recursive data-carrying discriminated union grammar and fixed-size arrays end to end across the TypeScript, Rust, and Python emitters.
