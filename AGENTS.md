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

## Memory-Lifetime Rules

Background: `crates/surge-ts-checker/MEMORY_REGIONS.md` ("Memory-lifetime
program") and `docs/MEMORY-OPTIMIZATION-REPORT.md`.

- MUST NOT retain canonical type-store payloads strongly without measured
  justification; the stores use `Weak` retention with monotonic, never-reused
  IDs.
- MUST register every `Drop`-requiring arena payload with the arena's
  destructor list exactly once (`pending_drops` in arena.rs).
- MUST NOT capture declaration span maps, value tables, diagnostics, flow
  state, or checker context in type declaration environments; environments
  hold stamp-deduplicated `Arc` table snapshots only.
- MUST NOT prune or shorten expansion-cache lifetimes (including
  `program_instantiations`) without full oracle evidence; cache lifetime is
  semantically load-bearing and pruning has measurably drifted zod
  diagnostics. Only true-death reclamation is approved.
- MUST NOT share resolution results keyed only on declaration identity when
  the result can depend on analysis pass, lexical/module/type-parameter
  scope, import or augmentation generation, recursion state, or resolution
  mode.
- MUST NOT use `git stash` for large cross-cutting memory work; commit and
  validate each memory stage independently.
