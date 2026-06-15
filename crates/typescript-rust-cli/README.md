# typescript-rust-cli

CLI entry point for the workspace checker.

v0.68 keeps the CLI shape stable while the checker expands emitted diagnostic coverage. CLI output should continue to reflect catalog-driven codes, spans, and line/column data without introducing package-resolution or lib.d.ts discovery.

Diagnostics are catalog-driven and rendered through the shared diagnostics crate.

## Modes

- Single-file mode: `typescript-rust-cli <file.ts>`
- Single-file mode (standalone): `typescript-rust-cli --ignoreConfig <file.ts>`
- Project mode: `typescript-rust-cli --project <tsconfig.json>`
- Project mode with deterministic parallel per-file checking: `typescript-rust-cli --project <tsconfig.json> --jobs 4`
- Compatibility report: `typescript-rust-cli --project <tsconfig.json> --compatReport`
- Stub external modules: `typescript-rust-cli --project <tsconfig.json> --stubExternalModules`

`--compatReport` always prints the discovered file count. If project mode loads
zero source files, the CLI emits a custom
`typescript-rust::project-has-no-source-files` diagnostic and the report
includes a visibility warning instead of silently returning an empty
comparison. The report also includes lightweight build provenance so it is
clear whether the output came from the current workspace binary: package
version, build profile, binary path, current directory, and workspace root.

## External Modules (v0.63)

By default, unresolved non-relative package imports emit TS2307.
`--stubExternalModules` suppresses non-relative TS2307 and inserts unknown type/value stubs.
This is a typescript-rust-only compatibility mode.

## Declaration Files & Built-ins (v0.69/v0.69.1/v0.70/v0.72/v0.72.1)

Loaded `.d.ts` files from project inputs participate in semantic checking.
Bare package imports (`pkg`, `@scope/pkg`) and exact subpaths resolve their `.d.ts` entrypoints via `types`, `typings`, `exports["types"]`, or `index.d.ts` fallback.
Explicit `paths` aliases and declaration-only package entries share the same internal resolved module map. Configured `compilerOptions.types` entries are resolved narrowly to `node_modules/@types/<name>` (scoped `@scope/pkg` maps to `@types/scope__pkg`), searching upward from the project root and loading the package's `types`/`typings`/exact `exports["."].types`/`index.d.ts` entrypoint as a dependency declaration; unlike imported dependency `.d.ts` files, these configured `@types` declarations also populate the ambient global scope. A configured type that cannot be found anywhere up the tree reports `TS2688`. This does not enable automatic inclusion of all visible `@types` packages, `typeRoots`, `typesVersions`, or wildcard `exports`. The CLI still does not discover full package resolution, wildcard `exports`, automatic `@types`, or full `lib.d.ts` parity. `baseUrl` resolution remains unsupported/deprecated. v0.85 introduces a generated default-lib foundation: it does not load the full official TypeScript lib files at runtime, but instead generates a small supported subset from the local TypeScript package and loads those generated declarations as ambient default libs. `noLib: true` disables the generated default libs. Full lib.d.ts parity remains future work.
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
cargo run -p typescript-rust-cli -- examples/basic.ts

# Standalone semantic file checking
cargo run -p typescript-rust-cli -- --ignoreConfig examples/basic.ts
```

## JSON output

- `--format json` prints diagnostic JSON in normal project or single-file mode.
- `--compatReport --format json` prints compatibility-report JSON.
- `--diagnosticProfile <tsc|native>` sets the diagnostic profile. The `tsc` profile strictly aligns with TypeScript's oracle baseline, while `native` aggressively suppresses noisy cascades at boundaries like `satisfies`. (Default: `tsc`)
- `--jobs` is project-mode infrastructure for deterministic per-file checking only. It keeps shared prepasses serial and merges diagnostics in loaded-file order. The default is `1`.
- `--showSpans` is a text-mode affordance; JSON output already carries spans and,
  when available, 1-based line and column numbers.
- `--maxDiagnostics` limits rendered diagnostics in normal diagnostic mode.
- `--compatReport` is a raw measurement surface: it reports totals, counts by code and file, parser-error grouping, loaded file counts, file-kind counts, and suppressed diagnostic totals where relevant. It does not perform semantic diagnosis. Raw parity analysis belongs in oracle output, fixtures, and implementation notes.
- The oracle comparison output prints the exact `typescript-rust` command and the explicit job count when `--rustJobs` is provided.

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
