# Adding Diagnostics Safely

This is a short pointer to the canonical guide in [`DIAGNOSTICS.md`](DIAGNOSTICS.md).

The important rules are:

- `TSxxxx` codes stay TypeScript-compatible.
- `typescript-rust::*` codes are for implementation-specific diagnostics.
- The catalog lives in `diagnostic-messages.json`.
- Generated Rust accessors live in `src/generated.rs`.
- Placeholder arity is validated before code is generated.
- Spans are still assigned in parser/checker code, not in the catalog.

Use `cargo run -p typescript-rust-diagnostics-codegen` to regenerate the accessors and snapshot fixture.
