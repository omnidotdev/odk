---
"@omnidotdev/odk": patch
---

Reject unsupported tagged-union output up front instead of emitting silently wrong code.

Data-carrying (tagged) unions are emitted only by the `rust` and `typescript` generate targets. Previously the other paths degraded silently: `odk generate -t python` collapsed a tagged union to a bare string enum, dropping every variant payload, and `odk compile` emitted an undiscriminated union alias, dropping the discriminator from the OpenAPI / JSON Schema output. The `rust` target also boxes only a direct self-reference, so a reference cycle through other types (mutual recursion) produced infinitely-sized Rust that failed at the consumer's `cargo build`.

Each of these now fails with a clear, actionable error that names the union (or the recursion cycle) and points to `-t rust,typescript`, rather than producing output that looks valid but is wrong. The supported paths are unchanged.
