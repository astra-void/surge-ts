# typescript-rust-cli

CLI entry point for the workspace checker.

## Modes

- Single-file mode: `typescript-rust-cli <file.ts>`
- Single-file mode (standalone): `typescript-rust-cli --ignoreConfig <file.ts>`
- Project mode: `typescript-rust-cli --project <tsconfig.json>`
- Compatibility report: `typescript-rust-cli --project <tsconfig.json> --compatReport`

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

## Workflow notes

- The CLI is pure Rust; it does not require Node tooling to build or test.
- `cargo test` does not require `pnpm install`.
