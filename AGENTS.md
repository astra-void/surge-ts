## Comment Policy

- Do not add obvious, redundant, or filler comments.
- Avoid comments that merely repeat what the code already says.
- Prefer clear names, small functions, and straightforward control flow over explanatory comments.
- Add comments only when they explain non-obvious intent, edge cases, compatibility constraints, performance tradeoffs, or safety-sensitive behavior.
- Do not add large header comments, decorative section comments, or generated-looking comment blocks unless explicitly requested.
- When modifying existing files, do not increase comment noise. Remove stale or misleading comments if they are directly related to the edited code.
- Comments should justify themselves; if a comment does not explain why the code exists or why it is written that way, do not add it.

## Search

- Prefer `rg` (ripgrep) over `grep` for all code searches. It respects `.gitignore`, is faster, and handles binary files safely.
- Fall back to `grep` only when `rg` is unavailable or a POSIX-compatible invocation is strictly required (e.g. inside a shell script that must be portable).

## Verification

- Rust crates: run the workspace tests with nextest.

  ```sh
  cargo nextest run --workspace
  ```

  Scope with `-p <crate>`, a substring filter (`cargo nextest run my_test_name`),
  or the filterset DSL (`-E 'test(my_test_name)'`). `fail-fast` is off by
  default (see `.config/nextest.toml`), so a run reports every failure.
  The first run after a rebuild can stall briefly while macOS Gatekeeper
  assesses the freshly built test binaries — environmental, not a hang.

  Fallback without nextest — build the test binary, then invoke it directly:

  ```sh
  cargo test --no-run 2>&1 | grep -oE '\(target/[^)]+\)' | tr -d '()' | xargs -I{} {}
  ```

  Use `-- --test-threads=1` or filter flags (e.g. `-- my_test_name`) after the binary path as needed.
- Oracle harness tests: `pnpm run oracle:test`.
- Single-target oracle check: `pnpm run oracle:compare -- --project <preset|tsconfig>`
  (or `--file <source.ts>`) to spot-check one fixture or project.
- Oracle compatibility sweep: after changes that can affect diagnostics, run
  `pnpm run oracle:sweep -- --all --maxDiagnostics 200` (or a targeted
  `pnpm run oracle:sweep -- --filter <group> --maxDiagnostics 200`, or
  `--discover <dir>` for projects outside the preset registry). A target fails
  the gate only on diagnostic code-count or file/code/line mismatch;
  message-text and span/column drift are reported but non-gating unless you pass
  `--strictMessages` / `--strictSpans`.
- Optional — benchmark harness tests: `pnpm run bench:test` (run when touching
  the benchmark harness).
- Do not edit fixtures, expected output, or checker semantics to make the sweep
  pass; report real regressions honestly instead.

## Performance and correctness guardrails

See [ARCHITECTURE.md](ARCHITECTURE.md) and
[docs/PERFORMANCE_INVARIANTS.md](docs/PERFORMANCE_INVARIANTS.md) for rationale.

- MUST NOT introduce any pattern in the "Prohibited patterns" list of
  docs/PERFORMANCE_INVARIANTS.md (deep-cloned `CheckerOptions`, per-file
  `CheckerContext` clones, `getenv` in hot loops, uninterned persistent type
  payloads, consumer-local lookup before dependency lexical scope, …).
- MUST NOT add environment-insensitive cross-pass or cross-module caches;
  cache keys must capture declaration, arguments, and environment identity,
  and preliminary-pass results must never install first-wins global state.
- MUST NOT cache degraded (`had_error`) results, fallback `Unknown`, or
  recursion-in-progress results program-wide; overload order and duplicates
  must be preserved exactly.
- REQUIRES BENCHMARK: changes to `crates/surge-ts-types/src/store.rs`,
  `crates/surge-ts-checker/src/context.rs`, hashing (`fx.rs`, hasher choices),
  caching, or canonicalization need an interleaved before/after benchmark on a
  real project (`pnpm real:trpc` style; single runs are noise — see
  MEMORY_REGIONS.md) plus the full oracle sweep.
- REQUIRES ORACLE PARITY: any change that can affect emitted diagnostics needs
  `pnpm run oracle:sweep -- --all --maxDiagnostics 200` before landing.
- New per-file checker state MUST join the `begin_file_check` reset; new
  program-lifetime caches MUST join the end-of-run teardown
  (`clear_program_type_caches`).
