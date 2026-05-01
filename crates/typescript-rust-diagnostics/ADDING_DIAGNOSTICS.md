# Adding Diagnostics Safely

This crate keeps a TypeScript diagnostic catalog separate from the set of diagnostics the checker can emit today.

## Support levels

- `DiagnosticSupport::CatalogOnly`: the diagnostic is tracked in the catalog, but no current checker or config path emits it yet.
- `DiagnosticSupport::Emitted`: the current checker or config loader can produce it today.

Use `CatalogOnly` by default when you add a new catalog entry. Switch to `Emitted` only when there is a real emitter in the codebase.

## Adding a new diagnostic

1. Add a new `TypeScriptDiagnosticKind` entry in `src/lib.rs`.
2. Fill in `code`, `key`, `category`, `message_template`, `argument_count`, and `support`.
3. Add a `Diagnostic::tsXXXX(...)` constructor only if the diagnostic is emitted today or the emitter is landing in the same change.
4. Keep constructors thin: they should call `Diagnostic::typescript(...)` and read metadata from the catalog.
5. Add or update a smoke fixture only when the diagnostic is actually emitted by the checker.
6. Update `tests/fixtures/typescript-diagnostics/catalog.snapshot.toml` if the catalog changes.

## Smoke fixtures

- Smoke fixtures should assert emitted `TS` codes, not catalog-only entries.
- Keep smoke cases small and focused on one diagnostic path.
- Avoid changing diagnostic order unless the checker behavior is intentionally changing.

## Snapshot updates

The snapshot fixture is a pinned verification list for the catalog currently in this repository.
Regenerate it from the catalog after changing entries, then rerun the diagnostics tests.

## Useful commands

```bash
cargo fmt
cargo check
cargo test -p typescript-rust-diagnostics
```

For checker-facing changes, also run the workspace smoke and CLI checks used by the repository.
