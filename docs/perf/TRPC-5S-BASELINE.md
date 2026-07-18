# tRPC 5-second program — Stage 0 baseline

Branch `perf/trpc-5s`, worktree `../surge-ts-trpc-5s`.
`TRPC_5S_BASELINE_COMMIT = d0e1b4cb2ff7333d0f792fd5e8a5b3288af7c3dd` (current `main`).

## Environment

- Apple M1 Pro, 10 cores (8P+2E), 16 GB RAM, macOS 27.0 (Darwin 27.0.0)
- rustc 1.94.0 (Homebrew), node v24.12.0, pnpm 11.13.0
- Allocator: system (xzone malloc). Build: `cargo build --release -p surge-ts-cli` (default release profile)
- Project: `.local-projects/trpc` (pinned tRPC checkout, ~4,933 program files incl. deps), tsconfig.json
- Command: `target/release/surge --project .local-projects/trpc/tsconfig.json --format json --jobs <1|auto>`
- Measurement: `/usr/bin/time -l`, interleaved jobs=1/jobs=auto, warm filesystem cache, fresh process

## Wall-time distribution (5 runs per mode, interleaved)

| mode | min | median | max | mean | stddev |
|---|---|---|---|---|---|
| jobs=1 | 19.48 | **20.52** | 21.43 | 20.45 | 0.70 |
| jobs=auto | 18.33 | **20.04** | 20.53 | 19.78 | 0.86 |

- user CPU ≈ 16.4–17.7 s, sys ≈ 2.4–3.1 s in both modes (single-core bound)
- peak phys footprint 1.97–2.07 GB, max RSS 1.47–2.10 GB (noisy, known)
- diagnostics: 2,190; JSON sha256 `4d69a2d5f549616083afa9c9e3bccc3484a8bdc96457988fd1f060b805b5ee59`
  byte-identical across all 10 runs and both modes (matches BENCHMARKS.md canonical hash)

## Phase attribution (CLI `--timings` + SURGE_TIMINGS stages + `sample` @1ms)

| Phase | Wall | Evidence |
|---|---|---|
| config + discovery + libs | 0.16 s | CLI timings |
| package_declaration_discovery | 2.50 s | CLI timings; 550,701 `stat` probes, 886 package.json reads, full re-parse of every source per fixpoint iteration (helpers.rs:799 ≈ 1.1 s parse) |
| import_graph_expansion | 2.23 s | CLI timings; re-parses every source per iteration from index 0 (serial ParserWorker); 3,698 file reads = 0.57 s I/O, rest CPU |
| path_mapping_resolution | 0.24 s | CLI timings |
| checker: parse | 0.27 s | stage `parsing` (parallel-capable; serial at jobs=1) |
| checker: ambient+global collection | 0.9 s | stages; profile mod.rs:568 ≈ 660 samples |
| checker: preliminary type-binding collection | 1.9 s | profile mod.rs:609 (timer: 1.49 s) |
| checker: PRELIMINARY module analysis | 4.5–5.2 s | stage + profile mod.rs:628 (3,505 samples) |
| checker: FINAL module analysis | 3.9–4.2 s | stage + profile mod.rs:778 (3,271 samples) |
| checker: import bindings ×3 + scope builds ×4 | ~0.45 s | profile mod.rs:692/718/866 |
| checker: module_local_values build | 1.0–1.6 s | stage + profile mod.rs:988 |
| checker: per-file check loop | 3.4 s | stage `check_phase` delta (profile window ended before this phase) |
| checker: teardown + finish | 0.45 s | stage `finish` |
| diagnostic rendering | 0.007 s | CLI timings — not a target |

## Why jobs=auto ≈ jobs=1

1. `resolve_worker_count` (program/mod.rs) sums `statements.len()` per file **after** the
   skipLibCheck declaration-AST release empties every dependency `.d.ts`, so tRPC's work
   estimate collapses and auto selects **1 check worker**.
2. Everything between parse and check — the dominant 11+ s binding/analysis pipeline —
   is serial on one `&mut CheckerContext`, and the ~4.7 s frontend loader loop is serial.
   Only parse (0.27 s) actually fans out.

## Cross-cutting costs (from `sample`, top-of-stack self time, ~16k samples)

- allocator (xzone malloc/free/realloc) ≈ **22%** of samples — allocation volume is the
  single largest self-time owner
- filesystem syscalls (`getattrlist`/`stat`/`read`/`open`) ≈ 12% — probe storms +
  canonicalize misses
- `canonicalize_if_exists*`: 5,999,949 calls/run, 99.7% cache hits, but ~1.8–2.2 s
  inclusive is spent in cache lookups (String hash + map). Dominant caller:
  `infer::types::cache::type_declaration_resolution_key` — every resolution-cache key
  allocates a canonicalized `String` + cloned name `String`
- SipHash (`core::hash::sip`) 368 self samples — a hot map still on the default hasher
- `memmove`/`memcmp` ≈ 7%

## Counter snapshot (SURGE_TIMINGS, jobs=1)

- canonical_type_store: function_requests 2,594,246 (unique 526,317);
  union_hits 2,371,316; parameter_list_hits 1,428,332; lock_contentions 0
- function_type_handle_copy 2,470,622 (substitution_changed 2,208,334)
- union_type_handle_copy 5,113,701 (substitution_changed 5,029,750)
- declaration_environment_store: requests 118,768 (hits 65,090)
- io: fs_existence_probes 550,701; package_json_reads 886; fs_read_dir_count 0

## Cold-cache note

All numbers above are warm-filesystem, fresh-process (the primary target mode). The
first-ever run of a fresh binary adds ~1 s (Gatekeeper assessment + cold page cache).
