---
"@omnidotdev/odk": patch
---

Security and dependency hardening for the Rust toolchain:

- Upgrade the WASM plugin runtime (extism 1.20 to 1.41, pulling wasmtime 37 to 49.0.5), clearing nine RUSTSEC advisories including sandbox escapes and Cranelift miscompilations.
- Bump the `zstd` family and `crossbeam-epoch` to patched versions.
- Replace the unmaintained `serde_yaml` with the maintained `serde_norway` fork (identical API).

No user-facing behavior change; all tests, examples, and the WASM plugin host validated. CI now runs `cargo audit`.
