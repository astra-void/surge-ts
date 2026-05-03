# typescript-rust-diagnostics

Diagnostic types, catalog accessors, and rendering helpers.

The source of truth for diagnostic metadata is [`diagnostic-messages.json`](diagnostic-messages.json).
Checked-in Rust accessors live in [`src/generated.rs`](src/generated.rs) and are regenerated from the catalog.

## Common entry points

- `Diagnostic::tsXXXX(...)` for TypeScript-compatible diagnostics
- `Diagnostic::typescript_rust_* (...)` for project-specific diagnostics
- `render_diagnostics(...)` for text output
- `cataloged_diagnostic_descriptors()` for catalog inspection

## Regeneration

```bash
cargo run -p typescript-rust-diagnostics-codegen
```

This updates the generated Rust accessors and the pinned TOML snapshot fixture.
