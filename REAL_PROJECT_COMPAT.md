# Real Project Compatibility

`v0.59.1` is still an instrumentation baseline for real-project compatibility,
not a claim that large TypeScript packages pass.

## Local workflow

- Do not commit third-party project source.
- Put disposable real-project experiments under `.local-projects/`.
- Keep local copies out of committed tests and fixtures.

Example:

```bash
mkdir -p .local-projects
cargo run -p typescript-rust-cli -- --project .local-projects/<project>/tsconfig.json --compatReport --maxDiagnostics 200
```

## What the report tells you

The compatibility report is a triage tool. It helps separate the first-order blockers from the noise:

1. Parser errors
2. Unsupported module syntax
3. Non-relative package imports
4. Missing global/lib symbols or unsupported generic syntax
5. Plain type mismatches

The report does not guarantee that a project is expected to pass.

## Current baseline

`v0.59` intentionally avoids:

- package resolution
- `node_modules` lookup
- `paths` / `baseUrl`
- lib.d.ts modeling
- declaration files
- project references
- incremental or watch behavior
- generic inference and generic classes
- enums and namespaces
- CommonJS or bundler semantics
- generic constraints enforcement
- generic call-site inference

The next phase should still be chosen from real report output, not from a fixed
feature wish list. Generic aliases and interfaces now support explicit type
arguments, defaults, and parser-safe constraints, but call-site type arguments
remain parser-safe and ignored by the checker. Likely follow-ups include:

- `v0.60 module syntax expansion`
- `v0.60 package import stubbing`
- `v0.60 declaration-file surface`
