## Comment Policy

- Do not add obvious, redundant, or filler comments.
- Avoid comments that merely repeat what the code already says.
- Prefer clear names, small functions, and straightforward control flow over explanatory comments.
- Add comments only when they explain non-obvious intent, edge cases, compatibility constraints, performance tradeoffs, or safety-sensitive behavior.
- Do not add large header comments, decorative section comments, or generated-looking comment blocks unless explicitly requested.
- When modifying existing files, do not increase comment noise. Remove stale or misleading comments if they are directly related to the edited code.
- Comments should justify themselves; if a comment does not explain why the code exists or why it is written that way, do not add it.

## Verification

- Rust crates: build the test binary first, then invoke it directly.

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
