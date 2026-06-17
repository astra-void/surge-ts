# Adding Diagnostics Safely

This is a short pointer to the canonical guide in [`DIAGNOSTICS.md`](DIAGNOSTICS.md).

For v0.68, prefer diagnostics that already have a real emission path, a focused fixture, a span policy, and a no-cascade policy where relevant. Catalog-only codes are fine as scaffolding, but they do not count toward emitted coverage.

The important rules are:

- `TSxxxx` codes stay TypeScript-compatible.
- `surge::*` codes are for implementation-specific diagnostics.
- The catalog lives in `diagnostic-messages.json`.
- Generated Rust accessors live in `src/generated.rs`.
- Placeholder arity is validated before code is generated.
- Spans are still assigned in parser/checker code, not in the catalog.

Use `cargo run -p surge-ts-diagnostics-codegen` to regenerate the accessors and snapshot fixture.
