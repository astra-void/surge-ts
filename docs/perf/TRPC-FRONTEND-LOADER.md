# tRPC frontend loader — session report (2026-07-20)

Branch `perf/frontend-loader-io`, commits `28c3113..4a2c28d` on top of `c1cf8c9`.
Hardware: Apple M1 Pro (10 cores), 16 GB, macOS 27.0, release profile.
Fixture: `.local-projects/trpc`. Canonical command:
`target/release/surge --project .local-projects/trpc/tsconfig.json --format json --maxDiagnostics 10000 --jobs auto`.

Lever (2) of the remaining path in [TRPC-5S-FINAL.md](TRPC-5S-FINAL.md): the
frontend, previously estimated at ~2.4 s with "the loader loop is a serial BFS;
specifier scanning and file reads parallelize; package resolution needs a
concurrent cache."

## What the frontend actually costs

`--timings` at session start, tRPC:

| phase | wall |
|---|---|
| config_project_loading | 0.06 s |
| file_discovery | 0.03 s (root reads, already parallel) |
| default_lib_loading | 0.01 s |
| package_declaration_discovery | 0.91 s |
| import_graph_expansion | 0.71 s |
| path_mapping_resolution | 0.24 s |
| **total** | **~1.86 s** |

Specifier scanning was already parallel (`specifier_scan::prefetch`, prior
session). The cost is syscalls, not parsing. Leaf histogram of a 2 s `sample`
over the frontend: `read` 815, `stat` 261, `__getattrlist` 188 (realpath),
`__open` 183 — everything else is single digits.

New `--timings` counters (commit `28c3113`) put numbers on it:

* `fs_existence_probe_io` **300 ms** over 204,919 memoized-miss probes (~1.47 µs
  each). Extensionless specifiers fan out to ~15 candidate paths.
* `expansion_read_io` **496 ms** over 3,698 files / 31 MB, all on the loader
  thread.
* `package_declaration_read_io` **106 ms** — the package-declaration reads are
  *not* where that phase's 0.9 s goes.

## Landed

* `28c3113` feat(loader): probe-time and package-declaration-read accounting.
* `5b76e4b` perf(loader): frontier-at-a-time import-graph BFS. Resolution stays
  serial (it mutates the probe cache and the known-file set); a wave's reads run
  on a pool and are appended in the order the file-at-a-time loop would have
  used, so `sources`/`inputs` order is unchanged by construction. Wave
  boundaries also expose files discovered *inside* the call to the specifier
  prefetch, which previously fell back to serial parsing.
* `4a2c28d` perf(loader): `read_package_json` hands out `Arc` handles instead of
  deep-cloning a `serde_json::Value` per cache hit (hits vastly outnumber the 886
  misses; large `exports` maps showed up as `IndexMap::clone` + `memmove`), and
  the four `package.json` existence checks now go through the memoized probe
  instead of issuing an uncached stat each time.

## Results (7-round interleaved A/B medians vs `c1cf8c9`)

| phase | base | after | Δ |
|---|---|---|---|
| package_declaration_discovery | 914 ms | 845 ms | −69 ms |
| import_graph_expansion | 707 ms | 313 ms | **−394 ms** |
| path_mapping_resolution | 241 ms | 221 ms | −20 ms |
| frontend total | ~1.86 s | ~1.38 s | **−483 ms** |

Wall (11-round interleaved, `--jobs auto`): base median 10.80 s → 10.52 s
(−0.28 s). Run-to-run noise is ±1.5 s, larger than the effect, so the phase
numbers above are the load-bearing measurement and the wall figure is only
consistent with them, not independent evidence.

## Validation

* `cargo fmt --check` clean; `cargo check --workspace` clean at each of the three
  commits (bisectable).
* `cargo nextest run --workspace`: 1578/1578.
* `pnpm run oracle:test`: 21/21.
* `pnpm run oracle:sweep -- --all --maxDiagnostics 200`: **97/97 pass, 0 gating
  mismatches, 5 message-drift-only** — identical to the documented baseline.
* Diagnostics byte-identical to `c1cf8c9` on trpc / zod / ky / ofetch at
  `--jobs 1` and `--jobs auto`; tRPC sha `4d69a2d5…5ee59`, 2,190 diagnostics
  (the pinned value), also identical across jobs 1/2/4/8/auto.

## What is left in the frontend (~1.38 s)

Re-profiled after the above, the leaf histogram is still syscall-dominated:
`read` 798, `stat` 391, `__getattrlist` 213, `__open` 205. The reads are now
spread across a pool; the serial remainder is stat and realpath.

1. **Existence probes (~300 ms, 204k stats).** A directory-listing cache would
   collapse the ~15-candidate fan-out per target into one `read_dir` per
   directory (`fs_read_dir_count` is 0 today — there is no directory caching at
   all). Two hazards make this more than a mechanical change and it was *not*
   attempted: `DirEntry::file_type()` does not follow symlinks, so pnpm's
   symlinked `node_modules` entries need a per-entry `metadata` fallback; and
   APFS is case-insensitive by default, so an exact-name set lookup would answer
   `false` where `metadata()` answers `true`. A fallback-to-stat on miss would
   preserve correctness but keep ~14/15 of the syscalls, since misses dominate —
   so this needs either a case-folded index plus a volume case-sensitivity probe,
   or it is not worth doing.
2. **`canonicalize` / realpath (~200 ms, `__getattrlist`).** `canonicalize_if_exists`
   memoizes on the exact `PathBuf`, so every distinct spelling of the same file
   pays a fresh realpath. A parent-directory-canonical + join scheme would reuse
   the resolved prefix across spellings.
3. **`canonicalize_if_exists_string`** allocates a `String` and rescans it with
   `.replace('\\', "/")` on every call, including cache hits, on the hottest
   loader paths. Caching the string form alongside the `PathBuf` is trivial and
   unmeasured.
4. **package_declaration_discovery (845 ms)** is now the largest frontend item
   and is still un-decomposed: its reads are only 106 ms, so the balance is
   probes, realpath, and the resolution logic itself. It needs its own
   instrumentation pass before any restructuring — a wave-parallel read
   transform there is sound (the resolution bookkeeping completes before each
   read) but would only be worth ~100 ms.
