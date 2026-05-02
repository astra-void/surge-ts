# typescript-rust-cli

CLI entry point for the workspace checker.

## Modes

- Single-file mode: `typescript-rust-cli <file.ts>`
- Project mode: `typescript-rust-cli --project <tsconfig.json>`
- Compatibility report: `typescript-rust-cli --project <tsconfig.json> --compatReport`

## JSON output

- `--format json` prints diagnostic JSON in normal project or single-file mode.
- `--compatReport --format json` prints compatibility-report JSON.
- `--showSpans` is a text-mode affordance; JSON output already carries spans and,
  when available, 1-based line and column numbers.
- `--maxDiagnostics` limits rendered diagnostics in normal diagnostic mode.

## Workflow notes

- The CLI is pure Rust; it does not require Node tooling to build or test.
- `cargo test` does not require `pnpm install`.
