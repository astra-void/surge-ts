# Tests

Behaviour tests run under [vitest](https://vitest.dev) against the release
`surge` binary. Rust unit tests for crate internals stay in `src/` and run with
`cargo test`.

```sh
pnpm test            # cargo build --release -p surge-ts-cli, then vitest run
pnpm test:vitest     # vitest run against the existing binary
pnpm test:rust       # cargo test --workspace (unit tests)
```

`SURGE_TS_BIN=/path/to/surge` pins the binary under test. Without it, the global
setup refuses to run when `target/release/surge` is older than any Rust source
under `crates/`.

## Fixtures

A fixture is one source file. The harness writes it into a scratch project
outside the workspace, runs `surge -p` and tsc 7.0.2 (the pinned `typescript`
package) on it, and requires the same diagnostics: file, line, column, and code,
as an unordered list. There are no stored expectations to go stale — tsc is the
expectation. tsc results are cached under `node_modules/.cache/surge-vitest/`,
keyed by the compiler version and the project contents.

| Directory                        | Runner                    |
| -------------------------------- | ------------------------- |
| `tests/smoke/`                   | `tests/smoke.test.ts`     |
| `tests/upstream/typescript-go/`  | `tests/upstream.test.ts`  |
| `tests/checker/<suite>/`         | `tests/checker.test.ts`   |
| `tests/cli/`                     | CLI behaviour tests       |

Fixture files are whatever `git ls-files --cached --others --exclude-standard`
lists, so `.js` emit that .gitignore excludes is never picked up.

### Directives

Directives are `// @name: value` lines, as in the TypeScript test suite. They stay in
the source, so line numbers match the fixture file.

- `// @filename: path` starts a new file of a multi-file fixture. Code before
  the first marker goes to a file named after the fixture.
- `// @<compilerOption>: value` sets a tsconfig compiler option. Names match
  case-insensitively; `true`/`false`/numbers are coerced; list options (`lib`,
  `types`, …) split on commas; a value starting with `{` or `[` is JSON. For
  a non-list option, a comma-separated value takes the first entry.
- `// @surge-compare: messages` also compares message text;
  `// @surge-compare: spans` also compares each diagnostic's end position, read
  from `--pretty true` underlines; `// @surge-compare: order` requires tsc's
  output order instead of comparing the diagnostics as a set.
- `// @surge-args: --flag value` passes extra arguments to surge only.
- `// @surge-expect: TS2322 surge::code` (or `none`) pins surge's codes in
  output order and skips tsc. Use it only for behaviour tsc has no counterpart
  for, such as the native diagnostic profile.
- `// @surge-skip: reason` registers the fixture as skipped.

Without an `@strict` directive, every fixture starts from the options the Rust
checker suites were written against: `strict: true` with `noImplicitAny`,
`noImplicitThis`, `strictBindCallApply`, `strictBuiltinIteratorReturn`,
`strictPropertyInitialization`, and `useUnknownInCatchVariables` off (see
`BASE_COMPILER_OPTIONS` in `harness/fixture.ts`). A fixture that sets `@strict`
gets plain tsc semantics for the whole strict family.

## Upstream cases

`tests/upstream/typescript-go/` vendors a subset of microsoft/typescript-go
compiler cases; see its README for the provenance rules.
