# typescript-rust-cli

CLI entry point for the workspace checker.

v0.68 keeps the CLI shape stable while the checker expands emitted diagnostic coverage. CLI output should continue to reflect catalog-driven codes, spans, and line/column data without introducing package-resolution or lib.d.ts discovery.

Diagnostics are catalog-driven and rendered through the shared diagnostics crate.

## Modes

- Single-file mode: `typescript-rust-cli <file.ts>`
- Single-file mode (standalone): `typescript-rust-cli --ignoreConfig <file.ts>`
- Project mode: `typescript-rust-cli --project <tsconfig.json>`
- Compatibility report: `typescript-rust-cli --project <tsconfig.json> --compatReport`
- Stub external modules: `typescript-rust-cli --project <tsconfig.json> --stubExternalModules`

## External Modules (v0.63)

By default, unresolved non-relative package imports emit TS2307.
`--stubExternalModules` suppresses non-relative TS2307 and inserts unknown type/value stubs.
This is a typescript-rust-only compatibility/triage mode.

## Declaration Files & Built-ins (v0.69/v0.69.1/v0.70/v0.72/v0.72.1)

Loaded `.d.ts` files from project inputs participate in semantic checking.
Bare package imports (`pkg`, `@scope/pkg`) and exact subpaths resolve their `.d.ts` entrypoints via `types`, `typings`, `exports["types"]`, or `index.d.ts` fallback.
Explicit `paths` aliases and declaration-only package entries share the same internal resolved module map. The CLI still does not discover full package resolution, wildcard `exports`, `@types`, or `lib.d.ts`. `baseUrl` resolution remains unsupported/deprecated. v0.72/v0.72.1 uses synthetic built-ins, not physical `lib.d.ts`, to reduce TS2304 noise. `Array<T>` and `ReadonlyArray<T>` are modeled enough to preserve element diagnostics. Utility types mostly suppress TS2304 and do not implement mapped/conditional type semantics yet. `noLib: true` disables synthetic built-ins. DOM, Node, `@types`, and true lib loading remain unsupported.
Default export, namespace import, named re-export, type-only re-export, star re-export, duplicate ambient module, and duplicate ambient global behavior is pinned rather than full TypeScript declaration merging.


## Single-file behavior

Positional file mode follows TypeScript CLI config behavior. If a `tsconfig.json` is discovered in the current working directory under the pinned policy and `--ignoreConfig` is absent, TS5112 is emitted before semantic checking.

`--ignoreConfig` intentionally bypasses config discovery and runs standalone semantic checking.

`--project` and `--ignoreConfig` cannot be combined in current policy.

Checker APIs do not emit TS5112.

Example docs:

```bash
# TypeScript-like CLI behavior; may emit TS5112
cargo run -p typescript-rust-cli -- examples/basic.ts

# Standalone semantic file checking
cargo run -p typescript-rust-cli -- --ignoreConfig examples/basic.ts
```

## JSON output

- `--format json` prints diagnostic JSON in normal project or single-file mode.
- `--compatReport --format json` prints compatibility-report JSON.
- `--showSpans` is a text-mode affordance; JSON output already carries spans and,
  when available, 1-based line and column numbers.
- `--maxDiagnostics` limits rendered diagnostics in normal diagnostic mode.

The JSON diagnostic shape stays stable across the catalog migration:

- `code`
- `message`
- `fileName`
- `line`
- `column`
- `span`

## Workflow notes

- The CLI is pure Rust; it does not require Node tooling to build or test.
- `cargo test` does not require `pnpm install`.
