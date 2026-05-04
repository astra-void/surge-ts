# Diagnostics Catalog

Diagnostics in this crate are catalog-driven.

v0.68.1 focuses on emitted diagnostics that are reachable from parser or checker code paths. Catalog-only entries may still exist as stepping stones, but they are tracked separately and must not be counted as compatibility coverage.

## Coverage Manifest

The `tests/fixtures/emitted-diagnostics.toml` manifest tracks test evidence for every diagnostic marked as `"support": "emitted"`.

- `support = "emitted"` means a real callsite plus test evidence (fixtures, span tests, or oracle coverage) exists.
- `support = "catalog-only"` means the diagnostic is listed but not officially emitted or verified yet.

The coverage tests (`cargo test -p typescript-rust-diagnostics --test coverage`) ensure that:
- Every `emitted` diagnostic has a manifest entry with concrete test evidence.
- `catalog-only` diagnostics don't accidentally drift into emitted code without a reason.
- Crucial compatibility diagnostics (like `TS2307`, `TS2314`, `TS2315`) are strictly enforced as emitted.

## Source of truth

The canonical catalog is [`diagnostic-messages.json`](diagnostic-messages.json).
The generator reads that file and writes checked-in Rust accessors to [`src/generated.rs`](src/generated.rs).

## Adding a diagnostic

1. Add an entry to `diagnostic-messages.json`.
2. Run `cargo run -p typescript-rust-diagnostics-codegen`.
3. Use the generated `Diagnostic::tsXXXX(...)` or `Diagnostic::typescript_rust_* (...)` accessor.
4. Add or update tests for the code, message, and span behavior.
5. Run `cargo test`.

## Policy

- `TSxxxx` codes are reserved for TypeScript-compatible diagnostics.
- `typescript-rust::*` codes are reserved for implementation-specific diagnostics.
- Categories are explicit in the catalog and map to the crate's `DiagnosticCategory` enum.
- Placeholder arity is validated from the message template.
- Spans are assigned at the parser or checker callsite, not in the catalog.
- TS2882 is catalog-backed for TypeScript's unresolved side-effect import
  diagnostic. The checker emits it for `import "pkg";` / `import "./missing";`
  when the module is not resolved, while ordinary unresolved imports continue
  to use TS2307.
- `--stubExternalModules` is checker policy, not catalog policy: it suppresses
  non-relative missing-module diagnostics, including TS2882, but not relative
  side-effect imports.

## Regeneration

```bash
cargo run -p typescript-rust-diagnostics-codegen
```

That command regenerates the Rust accessors and the pinned TOML snapshot fixture.
