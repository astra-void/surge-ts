# surge-ts-diagnostics

Diagnostic types, catalog accessors, and rendering helpers.

The source of truth for diagnostic metadata is [`diagnostic-messages.json`](diagnostic-messages.json).
Checked-in Rust accessors live in [`src/generated.rs`](src/generated.rs) and are regenerated from the catalog.

## Common entry points

- `Diagnostic::tsXXXX(...)` for TypeScript-compatible diagnostics
- `Diagnostic::surge_* (...)` for project-specific diagnostics
- `render_diagnostics(...)` for the project's custom text output
- `render_diagnostics_tsc(...)` (with `TscRenderItem` / `TscRenderOptions`) for
  `tsc`-compatible text output: `--pretty false` single-line form and
  `--pretty true` code-frame form, with optional `tsc`-matching ANSI color. It is
  a renderer only and never changes which diagnostics, spans, or messages are
  produced.
- `cataloged_diagnostic_descriptors()` for catalog inspection

## Regeneration

```bash
cargo run -p surge-ts-diagnostics-codegen
```

This updates the generated Rust accessors and the pinned TOML snapshot fixture.
