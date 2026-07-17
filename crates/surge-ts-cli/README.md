# surge-ts

CLI entry point for the workspace checker. `surge-ts` is a Rust-based TypeScript
noEmit compatibility checker; it aims for tsc-compatible diagnostics in
noEmit-style project checks.

The built binary is `surge` (`target/release/surge`). The crate is published
internally as `surge-ts-cli`, so `cargo` invocations use `-p surge-ts-cli`.

v0.68 keeps the CLI shape stable while the checker expands emitted diagnostic coverage. CLI output should continue to reflect catalog-driven codes, spans, and line/column data without introducing package-resolution or lib.d.ts discovery.

Diagnostics are catalog-driven and rendered through the shared diagnostics crate.

## Modes

- Single-file mode: `surge <file.ts>`
- Single-file mode (standalone): `surge --ignoreConfig <file.ts>`
- Project mode: `surge --project <tsconfig.json>`
- Project mode with deterministic parallel per-file checking: `surge --project <tsconfig.json> --jobs 4`
- Compatibility report: `surge --project <tsconfig.json> --compatReport`
- Stub external modules: `surge --project <tsconfig.json> --stubExternalModules`

`--compatReport` always prints the discovered file count. If project mode loads
zero source files, the CLI emits a custom
`surge::project-has-no-source-files` diagnostic and the report
includes a visibility warning instead of silently returning an empty
comparison. The report also includes lightweight build provenance so it is
clear whether the output came from the current workspace binary: package
version, build profile, binary path, current directory, and workspace root.

## Diagnostic output styles

The default human-readable output is **tsc-compatible**. The original
project-specific (Rust-style `error[TS....]` / ` --> `) output is preserved
behind an explicit flag, and JSON remains explicitly opt-in.

- `--diagnosticStyle <tsc|custom|json>` (alias: `--diagnostic-style`) selects the
  renderer. Default: `tsc`.
  - `tsc` — TypeScript-compiler-compatible text output (the default).
  - `custom` — the original `surge-ts` report style.
  - `json` — machine-readable diagnostics (equivalent to `--format json`).
- `--pretty <true|false|auto>` controls the multi-line `tsc` code-frame output.
  Default: `auto` (pretty when stdout is a TTY, like `tsc`).
- `--format json` continues to emit JSON (back-compat; used by the oracle
  harness). `--format text` maps to the default `tsc` style.

`--pretty false` matches `tsc`'s one-line-per-diagnostic output:

```text
src/index.ts(3,1): error TS2588: Cannot assign to 'a' because it is a constant.
```

`--pretty true` matches `tsc`'s multi-line code frame and summary footer:

```text
src/index.ts:3:1 - error TS2588: Cannot assign to 'a' because it is a constant.

3 a = 3;
  ~

Found 1 error in src/index.ts:3
```

When pretty output is enabled and color is active, ANSI escape sequences match
`tsc` (cyan file, yellow line/column, red `error`, gray code, inverse gutter,
red squiggle). Color follows the terminal by default and honors the standard
`NO_COLOR` / `FORCE_COLOR` environment variables for deterministic output.

`--showSpans` remains a debug affordance and forces the custom span renderer
even under the default `tsc` style.

`--diagnosticStyle` is independent of `--diagnosticProfile`: the former selects
how diagnostics are *rendered*, while the latter selects which diagnostics the
*checker* emits (oracle-aligned `tsc` vs. cascade-suppressing `native`).

The footer matches `tsc` exactly for the single-file/single-error case
(`Found 1 error in <file>:<line>`), the same-file multi-error case
(`Found N errors in the same file, starting at: <file>:<line>`), and the
multi-file case (`Found N errors in M files.` followed by the `Errors  Files`
table). Multi-line span underlining renders the span's starting line; rendering
every line of a multi-line span is deferred (the JSON output and oracle
comparison are unaffected).

## External Modules (v0.63)

By default, unresolved non-relative package imports emit TS2307.
`--stubExternalModules` suppresses non-relative TS2307 and inserts unknown type/value stubs.
This is a surge-ts-only compatibility mode.

## Declaration Files & Built-ins (v0.69/v0.69.1/v0.70/v0.72/v0.72.1)

Loaded `.d.ts` files from project inputs participate in semantic checking.
Bare package imports (`pkg`, `@scope/pkg`) and exact subpaths resolve their `.d.ts` entrypoints via `types`, `typings`, `exports["types"]`, or `index.d.ts` fallback.
Explicit `paths` aliases and declaration-only package entries share the same internal resolved module map. Configured `compilerOptions.types` entries and `/// <reference types="..." />` directives resolve through the effective type roots (`typeRoots` when set, otherwise ancestor `node_modules/@types` directories) and load the package's `types`/`typings`/exact `exports["."].types`/`index.d.ts` entrypoint as dependency declarations. TypeScript 6 does not implicitly include every visible `@types` package when `types` is absent; use `types: ["*"]` for wildcard discovery. Missing explicit configured types and missing reference-type directives report `TS2688`, with declaration-file reference diagnostics suppressed by `skipLibCheck`. The CLI still does not implement full package resolution, wildcard `exports`, or full `lib.d.ts` parity. `baseUrl` resolution remains unsupported/deprecated. v0.85 introduces a generated default-lib foundation: it does not load the full official TypeScript lib files at runtime, but instead generates a small supported subset from the local TypeScript package and loads those generated declarations as ambient default libs. `noLib: true` disables the generated default libs. Full lib.d.ts parity remains future work.
Default export, namespace import, named re-export, type-only re-export, star re-export, duplicate ambient module, and duplicate ambient global behavior is pinned rather than full TypeScript declaration merging.

`.tsx` files parse JSX syntax (elements, fragments, attributes, and `{...}`
expression containers) in expression position, and JSX expressions infer a
conservative `JSX.Element` stand-in so simple React-shaped files check without
cascades. Expression containers and capitalized component tags are still walked
for ordinary diagnostics (e.g. unresolved names report `TS2304`). This does not
imply JSX transforms, `JSX` namespace resolution, `JSX.IntrinsicElements` prop
validation, React globals, or DOM support.


## Single-file behavior

Positional file mode follows TypeScript CLI config behavior. If a `tsconfig.json` is discovered in the current working directory under the pinned policy and `--ignoreConfig` is absent, TS5112 is emitted before semantic checking.

`--ignoreConfig` intentionally bypasses config discovery and runs standalone semantic checking.

`--project` and `--ignoreConfig` cannot be combined in current policy.

Checker APIs do not emit TS5112.

Example docs:

```bash
# TypeScript-like CLI behavior; may emit TS5112
cargo run -p surge-ts-cli -- examples/basic.ts

# Standalone semantic file checking
cargo run -p surge-ts-cli -- --ignoreConfig examples/basic.ts
```

## JSON output

- `--format json` (or `--diagnosticStyle json`) prints diagnostic JSON in normal project or single-file mode.
- `--compatReport --format json` prints compatibility-report JSON.
- `--diagnosticProfile <tsc|native>` sets the diagnostic profile. The `tsc` profile strictly aligns with TypeScript's oracle baseline, while `native` aggressively suppresses noisy cascades at boundaries like `satisfies`. (Default: `tsc`)
- `--jobs` is project-mode infrastructure for deterministic per-file checking only. It keeps shared prepasses serial and merges diagnostics in loaded-file order. The default is `auto`.
- `--showSpans` is a text-mode affordance; JSON output already carries spans and,
  when available, 1-based line and column numbers.
- `--maxDiagnostics` limits rendered diagnostics in normal diagnostic mode.
- `--compatReport` is a raw measurement surface: it reports totals, counts by code and file, parser-error grouping, loaded file counts, file-kind counts, and suppressed diagnostic totals where relevant. It does not perform semantic diagnosis. Raw parity analysis belongs in oracle output, fixtures, and implementation notes.
- The oracle comparison output prints the exact `surge-ts` command and the explicit job count when `--rustJobs` is provided.

The synthetic builtin surface stays narrow and now serves as bootstrap coverage for the remaining gaps outside the generated default libs.

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
