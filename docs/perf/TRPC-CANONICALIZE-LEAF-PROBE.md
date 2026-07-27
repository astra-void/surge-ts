# Canonicalize leaf probes — session report (2026-07-28)

Branch `perf/frontend-realpath`, commits `ef5504f..3870b13` on top of `57e649a`.
Hardware: Apple M1 Pro (10 cores), 16 GB, macOS 27.0, release profile.
Fixture: `.local-projects/trpc`. Canonical command:
`target/release/surge --project .local-projects/trpc/tsconfig.json --format json --maxDiagnostics 10000 --jobs auto`.

Follow-up to [TRPC-FRONTEND-LOADER.md](TRPC-FRONTEND-LOADER.md) items (2)/(3)
(realpath reuse, string-form caching), plus its item (1) (directory-listing
probe cache), which was attempted and measured a dead end.

## The realpath shape

`std::fs::canonicalize` is `realpath(3)`: one `getattrlist` per path
component. Both canonicalization memos — the loader's
(`surge-ts-config/src/paths.rs`) and the checker's per-worker-thread cache
(`surge-ts-checker/src/paths.rs`) — paid that full walk per unique path
spelling. New counters (`ef5504f`) put the loader at 5,561 walks / ~223 ms
serial, and the checker's existing counters at 15,570 walks / ~400 ms summed
across threads (worker caches are per-thread and rebuilt every run).

When the parent directory's canonical form is already memoized, a single
`getattrlist(FSOPT_NOFOLLOW, ATTR_CMN_NAME | ATTR_CMN_OBJTYPE)` on
`canonical_parent/leaf` answers everything the walk would:

* call succeeds ⇔ the entry exists (`ENOENT`/`ENOTDIR` ⇔ realpath fails);
* `ATTR_CMN_OBJTYPE` says whether the leaf is a symlink (which still needs
  the full walk to chase the target — pnpm's `node_modules` links hit this
  once per package directory, then children ride the resolved prefix);
* `ATTR_CMN_NAME` is the on-disk name — the case-corrected form realpath
  reports on APFS's default case-insensitive volumes, which a naive
  `lstat`+join would get wrong (verified: `fs::canonicalize("dir/file.ts")`
  returns `Dir/File.ts`).

Parents resolve recursively (one walk per unique directory, then cached), so
a cold path costs one probe instead of one walk. Fallbacks keep behavior
byte-identical: trailing-separator spellings (`a/b/` — `Path::file_name`
hides them but realpath ENOTDIRs them), `..`-final paths, unresolved parents
(textual fallback, `resolved: false` flag), unsupported filesystems, and any
unexpected errno all take the old full walk. The probe primitive lives in
`surge_ts_types::leaf_probe` (the one crate both consumers depend on);
non-macOS keeps the old path entirely.

## Landed

* `ef5504f` feat: canonicalize memo-miss / full-realpath / miss-io counters
  in `--timings`.
* `45babb9` perf(loader): leaf-probe resolution + the string form cached
  alongside the canonical path (hits previously re-allocated and rescanned
  `replace('\\', "/")` on every call). Equivalence test covers case
  variants, symlinks, dangling links, ENOTDIR spellings, warm/cold cache.
* `86088f3` perf(checker): same scheme for the resolution-key cache
  (`CanonEntry { Arc<str>, resolved }`).
* `0dd8c7d` feat: existence probes attributed to the package-declaration
  phase per fixpoint iteration.
* `3870b13` feat: `fs_read_dir_io` timing next to the existing count.

## Results

Loader A/B (7-round interleaved vs `ef5504f`, noisy window — a test build ran
concurrently; counters are the load-bearing numbers):

| metric | base | after |
|---|---|---|
| canonicalize_full_realpaths | 5,561 | **10** |
| canonicalize_leaf_probes | 0 | 7,310 |
| canonicalize_miss_io | ~223 ms | **~30 ms** |
| frontend phase sum (pkg+graph+paths+config) | ~1.69 s | ~1.42 s |

Checker A/B (7-round interleaved, quiet window, loader-side as base):

| metric | base | after |
|---|---|---|
| canonicalize_syscalls | 15,570 | 17,715 (probes counted) |
| canonicalize_syscall_time (all threads) | 390 ms | **42 ms** |
| checking median | 10.25 s | 9.89 s |

All 28 A/B runs byte-identical (trpc sha `4d69a2d5…`, the pinned value).

## Validation

* `cargo nextest run --workspace`: 1579/1579 (adds the canonicalize
  equivalence test).
* `pnpm run oracle:test`: 21/21. (One earlier failure was an artifact of
  running the gate against a mid-edit working tree — the sweep rebuilds the
  binary from the tree; only gate a committed state.)
* `pnpm run oracle:sweep -- --all --maxDiagnostics 200`: 97/97, 0 gating,
  5 message-drift-only — identical to the documented baseline.
* Byte-identical diagnostics on trpc / zod / ky / ofetch at `--jobs 1` and
  `--jobs auto` vs the instrumented base.

## Dead end: the directory-listing probe cache (frontend item 1)

Attempted three times, fully measured, reverted:

1. List every probed directory once (`read_dir` → name index) and answer the
   ~212k existence probes (~327 ms) from memory. Result: probes → 3, but
   18,144 `read_dir`s cost **341 ms** — cost-neutral.
2. Threshold promotion (stat the first 4 uniques, list hot dirs only):
   51k stats (99 ms) + 16.2k listings (219 ms) — still neutral. Nearly every
   probed directory crosses the threshold.
3. Lean arena listing (no per-entry allocation, sorted index, lazy ASCII
   case-fold scan): 213 ms — the allocations were not the cost.

Root cause, microbenched on the trpc `node_modules` tree: macOS
`read_dir` costs ~29 µs/dir warm (raw `opendir`/`readdir` FFI is the same —
28.7 µs, so it is the syscall path, not std overhead) against ~2.4 µs/stat,
i.e. one listing ≈ 8–12 stats. The probed-directory distribution averages
~13 unique probes/dir with no long tail to exploit, so every variant lands
within ±5% of the stat baseline. On this filesystem the idea is
structurally break-even; do not re-attempt without a cheaper listing
primitive (e.g. `getattrlistbulk`) *and* evidence the distribution changed.

## Where the frontend stands (~1.3 s)

Quiet-window phase medians after landing: package_declaration_discovery
~650 ms, import_graph_expansion ~220 ms, path_mapping_resolution ~210 ms,
config_project_loading ~105 ms. Decomposition of the package phase (new
counters): probes 196k/~253 ms + reads ~80 ms + canonicalize ~0 — leaving
**~350–450 ms of resolution logic CPU** as the largest un-attacked frontend
item, ahead of any remaining I/O. Next session should profile inside
`resolve_package_declaration_entrypoints_with_cache` (candidate-generation
allocation, exports-map matching, per-importer re-resolution) rather than
chase syscalls further.
