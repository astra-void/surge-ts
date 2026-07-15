# Real Project Compatibility

This document measures `surge-ts` (a Rust-based TypeScript noEmit compatibility
checker) against real projects and baseline compilers. `TypeScript`/`tsc` refer
to the upstream compiler used as the oracle baseline. Historical version notes
below may refer to the project by its earlier `surge-ts` / `ts-rust`
labels; those are kept verbatim as measured-at-the-time records. The internal
Cargo crates are still named `surge-ts-*`; current report output and the
CLI binary are `surge-ts`.

## Current state

- auth-kit matches TypeScript exactly at 0/0 diagnostics under the measured
  command set.
- The oracle preset sweep is 76/76 under the normal gate (diagnostic code-count
  and file/code/line). Message-text and span/column drift are reported but
  non-gating unless `--strictMessages` / `--strictSpans` are passed. The
  `namespace-import-reexport-basic` preset pins re-export of namespace/named/
  default import bindings (`import * as z; export { z }`).
- The compact `diagnostics-pack` preset is green at exact 31/31 parity. It pins
  duplicate declaration / function-implementation parity (TS2451/TS2393 on every
  conflicting declaration), the TDZ TS2448+TS2454 pairing for block-scoped reads
  in the temporal dead zone, missing-return span placement (TS2355/TS2366 on the
  function/method name span), and use-site generic-arity spans (TS2314/TS2315).
  This is targeted emitted-diagnostic parity, not full TypeScript parity.
- Project mode loads the physical `lib*.d.ts` graph by default; the generated
  default-lib subset is a fallback when the `typescript` package is absent, not
  the normal project-mode source of truth. `noLib: true` keeps standard/DOM
  globals unavailable.
- Performance has been stabilized by a program-wide generic instantiation cache
  for context-free library/dependency declarations and by deferring the
  interface/alias payload clone in named-type resolution to a genuine cache miss.
  Regression fixtures (`generic-cache-dependency-instantiation-basic`,
  `generic-cache-module-source-not-persisted-basic`,
  `generic-cache-unresolved-argument-diagnostics-basic`) are registered as oracle
  presets. The `~0.20s` auth-kit medians recorded in the v1.2.5 notes below
  predate physical-lib-by-default and reflect the older generated-snapshot
  measurement state; on the current physical `.d.ts` path auth-kit measures
  higher (roughly `~0.6s` in the latest local measurement). Treat those older
  medians as historical and read current numbers from the latest
  `.bench/` measurement artifacts rather than the snapshot-era figures.

## unnamed (Next.js real-project measurement)

`unnamed` is the second real-project compatibility target after auth-kit: a local
Next.js App-Router project (`moduleResolution: bundler`, `jsx: react-jsx`,
`strict`, `paths: { "@/*": ["./*"] }`, `lib: dom/dom.iterable/esnext`, includes
`.next/types/**`). It is **not** a parity claim — it is a measured baseline used
to find the highest-impact compatibility blocker. Full Next.js / React / Prisma
parity remains out of scope.

- Command:
  `pnpm run real:unnamed`
  (`scripts/real-projects/measure-project.ts --project ../../nextjs/unnamed
  --name unnamed --allowMissing`), plus
  `pnpm run oracle:compare -- --project ../../nextjs/unnamed/tsconfig.json
  --maxDiagnostics 200`.
- Local project present: **yes** (`../../nextjs/unnamed`). Project source is never
  copied into this repo; `--allowMissing` keeps the script honest when absent.
- Artifacts: `.bench/real-projects/unnamed/` (`oracle-compare.json`,
  `compat-report.json`, `timings.txt`, `measurement.md`).

### Measured baseline

| Metric | Value |
| --- | ---: |
| TypeScript (tsc) diagnostics | 0 |
| surge-ts diagnostics (before this pass) | 259 |
| surge-ts diagnostics (after this pass) | 230 |
| raw oracle match | no (surge-ts over-reports; every surge-ts diagnostic is a false positive) |

tsc reports a clean `0`, so all surge-ts diagnostics are over-reports. This is an
honest "not close to parity" baseline, expected for a real Next.js app exercising
React/JSX contextual typing, generated route types, and namespaces — all
currently out of scope.

### Drift categories (surge-only, after this pass; tsc = 0)

| Code | Count | Category |
| --- | ---: | --- |
| TS7031 | 57 | JSX/React contextual callback param typing (implicit-any binding elements, e.g. `render={({ field }) => …}`) |
| TS2339 | 49 | property access on narrowed/union and unmodelled-lib receivers |
| TS7006 | 23 | JSX/React contextual callback param typing (implicit-any params, e.g. `onCheckedChange={(checked) => …}`) |
| TS2304 | 21 | namespaces / generated globals (`Prisma`, Next generated `Display`/`NextFontWithVariable`) |
| TS2536 | 19 | generated Next.js route types (`.next/types/validator.ts` `ParamMap` indexing) / namespace index access |
| TS2345 | 17 | string-literal-union argument widening/narrowing (`as const` lookup tables) |
| TS2305 | 11 | type-only re-exports of namespace values (`import type { z }`) and exported-type-with-unresolved-RHS (`Locale`) |
| TS2322 | 10 | assignability after narrowing |
| TS2349 | 10 | not-callable on unmodelled shapes |
| TS2741 | 6 | missing required property |
| TS2538 / TS2314 / TS2882 | 4 / 2 / 1 | misc index/generic-arity/side-effect-import |

Dominant blockers (TS7031 + TS7006 = 80, ~35% of drift) are React/JSX contextual
callback inference — explicitly out of scope for this pass (would require broad
contextual-typing/generic-inference work). Generated-route-type (TS2536) and
namespace (TS2304/`Prisma`) drift are also out of scope.

### Blocker selected and fixed

**Re-export of an imported binding** (`import * as z from "zod"; export { z }`).
This was the root cause of the zod `z` over-reports: every form file does
`import { z } from "zod"`, and zod's `index.d.cts` is
`import * as z from "./v4/classic/external.cjs"; export { z }`. surge-ts resolved
the namespace import for *expression* use (`z.object(...)` worked) but the
`export { z }` named-re-export path did not recognize a namespace/named/default
**import** binding as a valid local export source, so it emitted `TS2304 Cannot
find name 'z'` on the re-export and `TS2305 … has no exported member 'z'` on every
consumer. tsc accepts all three import forms re-exported by name.

Fix (smallest reproduction: `tests/compat-projects/namespace-import-reexport-basic`,
a relative-module shape derived from the drift category, not copied from
`unnamed`): the final module export-table build now threads the file's resolved
import symbols (`ModuleImportBindings.symbols`) into the `export { name }`
re-export lookup, so a re-exported namespace/named/default import resolves to its
real imported binding. The import symbols are used **only** for the named
re-export fallback — not the general initializer-inference environment — so
`export const X = ns.member()` initializer inference is unchanged (an earlier
broader version surfaced one cascade false positive on
`new $Class.getPrismaClientClass()(...)`; scoping the change to the re-export
lookup removed it). Value re-exports now resolve with their precise type, not a
fallback-to-`any`.

Impact on `unnamed`: 259 → 230 surge-only diagnostics (−29, the zod `z` cascade:
9 direct TS2305 plus ~18 downstream TS2339 and misc), **zero new false positives**.
auth-kit stays exact `0/0`; the oracle preset sweep is **76/76** (75 prior + the
new fixture) under the normal gate.

### Remaining next recommended fix

Type-only re-export of a namespace value (`import type { z } from "zod"` in the 3
`*-form.tsx` files, plus the exported-type `Locale` whose RHS
`(typeof routing.locales)[number]` is unresolved) — the type-side analogue of the
value re-export fixed here. After that, the dominant remaining drift
(TS7031/TS7006 React contextual callback inference) is the next high-impact but
much larger area; it should not be attempted as a "small blocker".

The version-tagged notes below are historical, recording how the checker reached
this state. Their wall-clock medians and "synthetic built-ins" / "generated
default-lib" descriptions reflect the measurement and lib model in effect at the
time, not necessarily current behavior.

`v0.60.1` is still an instrumentation baseline for real-project compatibility,
not a claim that large TypeScript packages pass. `v0.60` adds a TypeScript
oracle comparison harness on top of that baseline so we can measure the current
checker against a pinned compiler without changing the checker to chase parity.

v1.2.6 restores auth-kit to exact `0` diagnostics after a class-heritage
regression. `class`/`declare class` now resolve a single `extends` base, merging
the base's instance members (own and inherited) into the derived instance type
through the shared interface-heritage path, and `get`/`set` accessor members
lower to instance properties via the existing object/interface property path. A
declaration-file base that resolves to `any`, fails to resolve, or is an empty
unmodelled lib/dependency stub keeps the derived type open (no cascade) so
unmodelled DOM/Node bases like `Request` do not flood inherited access with
TS2339; an unresolved base in user source stays closed and still reports the
missing member. Rest parameters (`...args`) in class methods are now captured and
mark the signature variadic, fixing false TS2554 arity errors on calls like
`cookies.get(name)`. The auth-kit `NextRequest` (`extends Request` plus a
`get cookies()` accessor) shape is reproduced and pinned by the
`next-request-shape-authkit-regression` fixture. DOM/Node global parity and full
`@types` ingestion remain out of scope; the open-on-unmodelled-base policy is the
no-cascade stand-in.

v1.2.5 is a performance pass after v1.2.4, not a new TypeScript semantic phase.
No new TypeScript surface was added and, on the latest auth-kit measurement,
exact diagnostics remain 0 and raw oracle match stays yes. Four changes land:
(1) path canonicalization is memoized per run in both `surge-ts-checker`
and `surge-ts-config` so the repeated `std::fs::canonicalize` (realpath)
syscalls in type/module resolution and the project-discovery import-graph
fixpoint are paid once instead of every probe; (2) the instrumentation counters
that funnel through a single global `Mutex<ProgramCounters>` are gated behind
`--timings`, removing that lock from the hot symbol-lookup and table-clone paths
in normal runs (counters are still exact when `--timings` is set, which is how
the measurement harness collects them); (3) `SymbolTable` is now copy-on-write
(`Arc<HashMap<..>>` with `Arc::make_mut` on insert/remove), so the multi-pass
module-binding fixpoint's table clones share their map and only deep-copy on the
rare mutate-while-shared path, taking `symbol_table_entry_handle_copy_count`
from `86782` to `27698` and `symbol_info_handle_copy_count` from `92072` to
`32988` while the fixpoint logic is left untouched; and (4) relative-module
resolution is memoized per run (cleared at check start), so fixpoint passes after
the first reuse the resolved index instead of rebuilding and canonicalizing
candidate paths. The measured auth-kit medians improve from v1.2.4's `0.80s` /
`0.78s` to roughly `0.20s` / `0.19s` for `jobs=1` / `jobs=4` (stable floor near
`0.18s`), now ahead of `tsgo` and well ahead of `tsc`. Profiling showed the
dominant pre-fix cost was uncached `realpath`, not type-payload cloning; the
remaining hot cost is the multi-pass binding/resolution recompute itself, which
is left for a future, correctness-sensitive fixpoint-reduction pass. No hot
allocator mutex was introduced and the prior handle-backed migrations are
preserved.

v1.2.4 is a performance recovery / stabilization pass after v1.2.3, not a new
TypeScript semantic phase. No new TypeScript surface was added. v1.2.3
`SymbolInfo` shared-handle storage is preserved while function-local variable
checking borrows visible symbols instead of cloning whole tables, function
signature setup lazily clones parameter scopes only when parameter initializers
need them, and `ScopeStack` restores per-frame visible-symbol shadows instead
of eagerly rebuilding the flat visible table on every pop. On the latest
auth-kit measurement, exact diagnostics remain 0 and raw oracle match stays
yes. Module-export reductions from v1.2.2 are preserved with
`function_type_copy_from_module_export_count=0` and
`union_type_copy_from_module_export_count=0`. The measured auth-kit medians are
`0.80s` at `jobs=1` and `0.78s` at `jobs=4`, improved from v1.2.3's
`0.85s`/`0.88s` but not fully back to v1.2.2's `0.67s`/`0.65s`. Current
handle counters are `function_type_handle_copy_count=2349`,
`union_type_handle_copy_count=1181`, `object_type_payload_deep_clone_count=0`,
`function_type_payload_deep_clone_count=0`, and
`union_type_payload_deep_clone_count=0`. `scope_or_context` attribution remains
near zero at `1` for function handles and `11` for union handles. Remaining
symbol/scope pressure is reported honestly:
`symbol_info_handle_copy_count=92072`, `symbol_table_clone_count=9143`,
`symbol_table_entry_handle_copy_count=86782`,
`scope_stack_visible_rebuild_count=0`, and
`scope_stack_visible_symbol_handle_copy_count=513`. Remaining
`symbol_info_payload_deep_clone_count=6` is rare replacement/construction work.
TypeDeclarationTable/ObjectType/FunctionType/UnionType handle-backed migrations
remain preserved, and no hot allocator mutex was introduced.

v1.2.1 is an attribution-first stabilization pass, not a new semantic phase.
No new TypeScript surface was added. On the latest auth-kit measurement, exact
diagnostics remain 0 and raw oracle match stays yes, but the total handle-copy
counts did not drop yet: `function_type_handle_copy_count=946298` and
`union_type_handle_copy_count=10047`. Wall-clock changed from v1.2's
`0.98s`/`1.00s` to `0.95s`/`0.92s` for `jobs=1`/`jobs=4`, so the `jobs=4`
regression is gone and `jobs=1` improved slightly. The current timing dump now
shows `type_declaration_collection=650.416ms`, `module_binding=361.276ms`,
`per_file_statement_checking=39.184ms`, `flow_narrowing=45.141ms`,
`function_declaration_checking=37.463ms`, `object_literal_checking=8.381ms`,
`call_expression_checking=1.261ms`, and `assignability_checking=0.463ms`. The
new attribution surface is
materially better: function copies are mostly from `module_export=378735`,
`function_body_setup=211137`, and `scope_or_context=194561`, with
`function_type_copy_unattributed_count=156626` still remaining; union copies are
mostly from `module_export=3155`, `scope_or_context=2248`, and
`function_body_setup=1970`, with `union_type_copy_unattributed_count=1672`
remaining. Both payload deep clone counts stay at zero. The next phase should
optimize one of those attributed sources instead of broadening the clone
surface again.

v1.2 is a performance-first stabilization pass, not a new semantic-expansion
phase. No new TypeScript surface was added. On the latest auth-kit measurement,
exact diagnostics remain 0 and raw oracle match stays yes, but the handle-copy
reduction was still modest: `function_type_handle_copy_count=946298` and
`union_type_handle_copy_count=10047`. `jobs=1` is `0.98s` and `jobs=4` is
`1.00s`, so wall-clock time had not improved yet. The timing dump now shows
`type_declaration_collection=649.339ms`, `module_binding=364.431ms`,
`per_file_statement_checking=39.620ms`, `flow_narrowing=44.889ms`,
`function_declaration_checking=37.614ms`, `object_literal_checking=8.720ms`,
`call_expression_checking=1.268ms`, and `assignability_checking=0.461ms`. The
current attribution surface only showed `55` function copies and `100` union
copies from expression identifier lookups; the remaining copies still sat
elsewhere in the function-body and call-checking paths.

v1.1 supports narrow generic indexed access after concrete substitution,
including `T["key"]`, `T[K]`, and `T[keyof T]` when the receiver/key have
been substituted to concrete types. Fully unresolved generic indexed access
and constraint enforcement remain unsupported.
`indexed-access-basic`, `mapped-types-basic`, `type-operators-basic`,
`generic-call-inference-basic`, `generics-basic`, and
`contextual-callback-object-properties-basic` still match TypeScript, auth-kit
stays exact at 0 diagnostics, raw oracle match stays yes, compatReport
diagnosticsTotal stays 0, and `suppressedRustOnlyDiagnosticsTotal` remains 20
in the tsc-profile report. `ObjectType`, `FunctionType`, and `UnionType`
remain handle-backed, `TypeDeclarationTable` stays arena-backed from v0.96,
and no hot allocator mutex was introduced.

v0.99 completes the composite-type handle slice by moving `UnionType` payloads
behind shared handles while preserving the earlier `ObjectType` and
`FunctionType` migrations and the v0.96 `TypeDeclarationTable` arena-backed
payloads. The auth-kit measurement stays exact at 0 diagnostics, raw oracle
match stays `yes`, `compatReport diagnosticsTotal=0`, and
`suppressedRustOnlyDiagnosticsTotal=20`. On the measured auth-kit project, the
benchmark medians are `1.12s` at `jobs=1` and `1.11s` at `jobs=4` for `tsc`,
`0.43s` and `0.43s` for `tsgo`, `0.55s` and `0.53s` for
`tsgo-singleThreaded`, and `0.95s` and `0.92s` for `ts-rust`, so this slice is
structural cleanup rather than a dramatic wall-clock win. The release timing
dump now shows `type_declaration_collection=632.120ms`,
`module_binding=350.221ms`, `per_file_statement_checking=38.130ms`,
`flow_narrowing=43.167ms`, `function_declaration_checking=36.444ms`,
`object_literal_checking=8.423ms`, `call_expression_checking=1.208ms`, and
`assignability_checking=0.462ms`. The counters now show
`checker_arena_alloc_count=25491`,
`arena_object_type_payload_alloc_count=1993`,
`object_type_payload_deep_clone_count=0`,
`object_type_clone_count=280`,
`object_type_id_copy_count=280`,
`function_type_payload_alloc_count=2461`,
`function_type_payload_deep_clone_count=0`,
`function_type_handle_copy_count=946413`,
`function_type_clone_count=946413`,
`union_type_payload_alloc_count=1851`,
`union_type_payload_deep_clone_count=0`,
`union_type_handle_copy_count=10516`,
`union_type_clone_count=100`,
`type_clone_count=771`. This is still a handle-backed slice, not a full
type-arena migration, and no hot allocator mutex has been introduced.

v0.82 is a project visibility and file-discovery hardening phase. It does not
claim full real-project parity. The goal is to make silent zero-file project
comparisons impossible, especially when `tsc` sees `.tsx`, `.mts`, `.cts`,
`.d.ts`, and nested `examples/**` inputs that the Rust loader might otherwise
miss. `.tsx` visibility is not the same as JSX or React type support. A later
parser-safe JSX slice adds JSX element/fragment/attribute parsing and a
conservative `JSX.Element` inference (walking `{...}` containers and component
tags for ordinary diagnostics) without `JSX` namespace resolution, intrinsic
prop validation, React globals, or the JSX transform.

v0.97.1 stabilizes the v0.97 object-slice landing instead of starting a new
arena/type-IR phase. `contextual-callback-object-properties-basic` and
`mapped-types-basic` now match TypeScript again, auth-kit stays exact at 0
diagnostics, raw oracle match stays yes, compatReport diagnosticsTotal stays
0, and `suppressedRustOnlyDiagnosticsTotal` remains 20 in the tsc-profile
report. `ObjectType` payloads still live behind shared handles, `FunctionType`
and `UnionType` remain value-owned, and no UnionType/FunctionType migration
has started.

v0.97 keeps auth-kit exact at 0 diagnostics and moves `ObjectType` payloads
onto shared handles instead of repeating deep clones of the property map.
`ObjectType` construction now goes through a checker-side allocation helper,
so `Type::Object` clone paths copy handles while object payload deep clones
drop to zero. On the measured auth-kit project, the benchmark medians are
`0.94s` at `jobs=1` and `0.90s` at `jobs=4`, with timing buckets of
`type_declaration_collection=1167.696ms`, `module_binding=542.743ms`,
`import_binding_resolution=407.085ms`, `per_file_statement_checking=305.801ms`,
`function_declaration_checking=294.843ms`, and `flow_narrowing=372.755ms`.
The counters now show `checker_arena_alloc_count=23067`,
`arena_declaration_key_alloc_count=10538`,
`arena_type_declaration_payload_alloc_count=10538`,
`arena_object_type_payload_alloc_count=1991`,
`type_declaration_payload_deep_clone_count=15319`,
`object_type_payload_deep_clone_count=0`,
`type_clone_count=763`,
`object_type_clone_count=275`,
`object_type_id_copy_count=275`,
`union_type_clone_count=98`,
`symbol_name_clone_count=0`,
`string_key_clone_count=0`,
`flow_local_name_clone_count=0`,
`string_path_lookup_count=30470`, and
`canonical_file_id_lookup_count=14574`. Exact diagnostics stayed at 0, raw
oracle match stayed yes, compatReport diagnosticsTotal stayed 0, and
`suppressedRustOnlyDiagnosticsTotal` is 20 in the tsc-profile report. This is
still a handle-backed slice, not a full type-arena migration: `FunctionType`
and `UnionType` remain value-owned.

v0.96 is a confirmed real payload migration, not a key-only landing. The
checker now stores `TypeDeclarationInfo` payloads as arena-owned handles behind
`TypeDeclarationId` entries, while declaration names continue to live in
arena-backed `ArenaStr` keys. The arena is program-local and cloned read-only
into worker contexts, so allocation happens during lowering and table clone
paths copy IDs/handles instead of payload bodies. On the measured auth-kit
project, the benchmark medians moved from `3.04s` to `2.36s` at `jobs=1` and
from `2.54s` to `2.10s` at `jobs=4`. The timing dump now shows
`type_declaration_collection` at `1140.712ms`, `module_binding` at
`457.063ms`, `import_binding_resolution` at `290.375ms`,
`per_file_statement_checking` at `656.381ms`, `function_declaration_checking`
at `617.091ms`, and `flow_narrowing` at `598.324ms`. The counters now show
`checker_arena_alloc_count=21076`,
`arena_declaration_key_alloc_count=10538`,
`arena_type_declaration_payload_alloc_count=10538`,
`type_declaration_table_clone_count=4`,
`type_declaration_id_copy_count=1579`,
`type_declaration_payload_deep_clone_count=15319`,
`type_declaration_entries_merged_total=863`, `type_clone_count=763`,
`object_type_clone_count=275`, `union_type_clone_count=98`,
`symbol_name_clone_count=0`, `string_key_clone_count=0`,
`flow_local_name_clone_count=0`, `string_path_lookup_count=30470`, and
`canonical_file_id_lookup_count=14574`. Exact diagnostics stayed at 0, raw
oracle match stayed yes, and compatReport diagnosticsTotal stayed 0. Reviewers
should inspect `crates/surge-ts-checker/Cargo.toml`,
`crates/surge-ts-checker/src/arena.rs`,
`crates/surge-ts-checker/src/lib.rs`,
`crates/surge-ts-checker/src/symbols/type_declarations.rs`,
`crates/surge-ts-checker/src/program.rs`,
`crates/surge-ts-checker/ARENA_ID_PLAN.md`,
`REAL_PROJECT_COMPAT.md`, and
`.bench/auth-kit-measurement.md` to verify the landing surface. The arena-backed
slice now covers declaration keys plus declaration payloads, and payload
cloning is no longer part of table cloning, even though direct payload clone
sites are still tracked separately.

v0.95 keeps auth-kit exact at 0 diagnostics and lands the first live
`oxc_allocator` slice in the checker. `TypeDeclarationTable` now interns
declaration names into arena-backed `ArenaStr` keys through a program-local
`CheckerArena`, while declaration payloads remain value-owned. On the measured
auth-kit project, the benchmark medians moved from
`3.1008863750000017s` to `3.04s` at `jobs=1` and from `2.568661s` to `2.54s`
at `jobs=4`. The timing dump now shows `type_declaration_collection` at
`2222.851ms`, `module_binding` at `1056.509ms`,
`import_binding_resolution` at `924.218ms`, `per_file_statement_checking` at
`666.348ms`, `function_declaration_checking` at `626.823ms`,
`flow_narrowing` at `617.100ms`, and `declaration_table_merging_cloning` at
`2.504ms`. The counters now show `checker_arena_alloc_count=10538`,
`type_arena_alloc_count=10538`, `type_declaration_table_clone_count=4`,
`type_declaration_entries_cloned_total=1579`,
`type_declaration_entries_merged_total=863`, `type_clone_count=763`,
`object_type_clone_count=275`, `union_type_clone_count=98`,
`symbol_name_clone_count=0`, `string_key_clone_count=0`,
`flow_local_name_clone_count=0`, `string_path_lookup_count=30470`, and
`canonical_file_id_lookup_count=14574`. Exact diagnostics stayed at 0, raw
oracle match stayed yes, and compatReport diagnosticsTotal stayed 0. The
arena-backed slice covers declaration-key interning only; payload cloning still
happens on `TypeDeclarationInfo` and the remaining module/function/flow paths.

v0.94 stops parsing and lowering generated default libs at runtime by using the
checked-in snapshot tables for the generated core/DOM subset, and it fixes a
package declaration classification bug so `node_modules/**/dist/*.d.ts` files
are treated as dependency declarations instead of generated libs. On the
measured auth-kit project, `generated_default_lib_files=2`,
`parsed_generated_default_lib_files=0`, `generated_default_lib_parse_time=0.000ms`,
and `generated_default_lib_lower_time=0.036ms`. `dependency_declaration_parse_time`
measured `4.907ms` and `dependency_declaration_lower_time` measured `0.910ms`.
The benchmark medians moved from `3.262684000000001s` to `3.1008863750000017s`
at `jobs=1` and from `2.767987000000001s` to `2.568661s` at `jobs=4`. Exact
diagnostics stayed at 0, raw oracle match stayed yes, and
`compatReport diagnosticsTotal` stayed 0. The remaining runtime work is still
import resolution, declaration collection, and flow/function checking. The
live `TypeArena` slice has not landed yet; its counters remain zero and the
next phase should decide whether to thread `FileId`/`ModuleId` first or start a
narrow arena-backed composite type slice.

v0.93 keeps auth-kit exact at 0 diagnostics and replaces the remaining string-key
hot paths with Arc-backed program-local interning for symbol, type-declaration,
flow-local, and file-identity keys. On the measured auth-kit project, the
benchmark medians moved from `2.720818041999999s` to `2.65s` at `jobs=1` and
from `2.2235199169999977s` to `2.15s` at `jobs=4`. The timing dump now shows
`type_declaration_collection` at `1.152298s`, `module_binding` at `483.614ms`,
`import_binding_resolution` at `317.398ms`, `per_file_statement_checking` at
`653.938ms`, `function_declaration_checking` at `614.590ms`, and
`flow_narrowing` at `597.704ms`. The symbol/string clone counters dropped to
zero, while `string_path_lookup_count=30503` and
`canonical_file_id_lookup_count=14574` stayed flat, so file/module identity
lookup is the next measurable bottleneck. Exact diagnostics stayed at 0, raw
oracle match stayed yes, and compatReport diagnosticsTotal stayed 0.

## ky (Fetch-API real-project parity)

`ky` is [sindresorhus/ky](https://github.com/sindresorhus/ky) 2.0.2: ~29 small
Fetch-API / DOM-typed source files, `tsconfig` extending `@sindresorhus/tsconfig`
(lib `DOM`+`DOM.Iterable`+`ES2023`, `exactOptionalPropertyTypes`, target
`esnext`). **`tsc` reports 0 diagnostics on it**, so it is a strict
false-positive corpus: every surge-ts diagnostic is a known-wrong over-report.
Unlike `unnamed` (a measured baseline), ky is now used as a **parity claim and a
regression gate**.

- Command: `pnpm run real:ky`
  (`measure-project.ts --project .local-projects/ky --name ky --allowMissing`),
  plus `pnpm run oracle:compare -- --project .local-projects/ky/tsconfig.json
  --failOnMismatch`.
- Local project present: gated. `.local-projects/` is gitignored — ky is **not
  vendored**. The source is never copied into this repo; `--allowMissing` keeps
  the script honest when absent.
- Artifacts: `.bench/real-projects/ky/` (`measurement.md`, `compat-report.json`).

### Current measurement (2026-06-20): 0/0

- TypeScript total diagnostics: **0**. surge-ts total diagnostics: **0**.
- code-count match: **yes**; file/code match: **yes**; only-TypeScript: 0;
  only-surge-ts: 0. surge-ts matches tsc exactly.
- Regression gate: `pnpm run real:ky:test` (also run by `pnpm run real:test`)
  runs the surge↔tsc comparison and fails on any drift from 0/0. It **skips**
  when ky or the `typescript` package is absent (mirroring the physical-lib rust
  tests). The specific patterns that were fixed are additionally pinned as
  cargo fixtures: `tests/compat-projects/physical-lib-new-promise-executor-basic`
  and `tests/compat-projects/physical-lib-required-omit-pick-basic`, plus the
  `cli_*` regressions in `crates/surge-ts-cli/tests/project_mode.rs`.

### Suppression / stub transparency (not yet audited)

Source-level parity is 0/0, but the compatReport shows three non-zero suppression
counters on ky that gate the parity claim and need a transparent audit (tracked
in `crates/surge-ts-checker/SUPPRESSED_DIAGNOSTICS_AUDIT.md`):

- `suppressedRustOnlyDiagnosticsTotal = 15` — `surge::*` diagnostics
  (parser/internal limits, never TS codes) suppressed before user output.
- `suppressedDeclarationDiagnosticsTotal = 23` — diagnostics inside declaration
  (`.d.ts`) files suppressed (trusted upstream lib/dependency declarations).
- `externalModuleStubs.total = 1` — one imported module resolved to a stub
  rather than a real declaration.

These do not affect the source-file comparison, but a product-grade "matches
tsc on ky" claim must confirm none of them hides a real source-level miss.

### History (false-positive burn-down)

ky was adopted mid-2026 as a false-positive corpus. The over-report count fell
across a sequence of targeted checker fixes (each verified oracle-clean):
**~42 (post-runaway) → 39 → 36 → 22 → 16 → 13 → 6 → 3 → 2 → 0**. The
`36`-remaining and intermediate states are kept here as historical records of
that burn-down; they were real measured over-report counts at the time, not the
current parity. The final clusters cleared on 2026-06-20 were: a `typeof
<importedValue>` module-value fallback, an `any`-typed callee being callable,
OR-of-guards + `ArrayBuffer.isView` narrowing, contextual `new Promise<void>`
generic-constructor inference (+ function→`Function` assignability), and the
`Required`/`Readonly` utility resolution (+ generic-context TS2538 suppression
and `&&`-chain truthy-property narrowing). The detailed root-cause taxonomy
lives in the working notes, not this doc.

## ofetch (Fetch-API real-project measurement)

`ofetch` is [unjs/ofetch](https://github.com/unjs/ofetch): a small Fetch-API
wrapper, 7 source files under `src/` plus a `test/` suite, `tsconfig` with
`module`/`moduleResolution: NodeNext`, `strict`, `verbatimModuleSyntax`,
`isolatedModules`, `isolatedDeclarations`, `composite`. **`tsc` reports 1
diagnostic** (`TS5107` — `esModuleInterop=false` is deprecated), so it is a near
false-positive corpus rather than a strict 0-baseline.

- Command: `pnpm run real:ofetch`
  (`measure-project.ts --project .local-projects/ofetch --name ofetch
  --allowMissing`), plus `pnpm run oracle:compare -- --project
  .local-projects/ofetch/tsconfig.json --maxDiagnostics 300`.
- Local project present: gated. `.local-projects/` is gitignored — ofetch is
  **not vendored**; `--allowMissing` keeps the script honest when absent.

### Measured baseline

| Metric | Before this pass | After this pass |
| --- | ---: | ---: |
| TypeScript (tsc) diagnostics | 1 (`TS5107`) | 1 (`TS5107`) |
| surge-ts diagnostics | 5 | 2 |
| surge-ts over-reports (false positives) | 5 | 2 |

tsc's single diagnostic is the `esModuleInterop=false` deprecation
(`TS5107`), which surge does not model (no compiler-option deprecation
diagnostics exist yet) — an under-report, not a false positive.

### False positives fixed this pass (5 → 2)

Three checker over-reports were root-caused and fixed (each verified against the
oracle preset sweep, still green):

- **`TS2339` on primitive literal index access** — `path[0]` (with `path:
  string`) reported `Property 'path' does not exist on type 'string'`. The
  statement-level index-access evaluator emitted a missing-property error for any
  literal index on any receiver, naming the *receiver* as the absent property.
  Restricted to object-like receivers (`Object`/`Function`/`Reference`):
  primitives carry an apparent type with index signatures (`string[number] ->
  string`), so a literal index there is never a `TS2339`.
  ([`checks/expr.rs`](crates/surge-ts-checker/src/checks/expr.rs))
- **`TS2349` "not callable" after truthy narrowing** — `if (hooks) { hooks(ctx) }`
  with `hooks: Hook | undefined` reported the call as not callable. The
  positive-branch narrowing handled `typeof`/`instanceof`/`Array.isArray`/
  discriminant guards but not a bare-identifier truthy guard, so `undefined` was
  never dropped and the callee stayed `Hook | undefined`. Added bare-identifier
  truthy narrowing (`remove_nullish` on the true branch); the existing `!guard`
  unwrap routes `if (!x)` else/fall-through through the same path.
  ([`checks/function/narrowing.rs`](crates/surge-ts-checker/src/checks/function/narrowing.rs))
- **`TS2339` for `Object.prototype` members on object/named types** —
  `error.toString()` reported `Property 'toString' does not exist on type
  'Error'`. Object and named-interface apparent types now expose the
  `Object.prototype` members.
  ([`types/object.rs`](crates/surge-ts-types/src/object.rs))

### Remaining (2): `node:*` import resolution via transitively-loaded `@types/node`

Both remaining over-reports are `TS2591` on `import … from "node:stream"`
(`src/fetch.ts`, `test/index.test.ts`). tsc resolves these because `@types/node`
is **loaded transitively** — a dependency `.d.ts` (vitest/vite) carries a `///
<reference types="node" />` that pulls the package into the program, which makes
its `declare module "node:stream"` visible. surge does not follow that transitive
type-reference chain, so the specifier stays unresolved and surge emits the
install hint.

A stub-resolution heuristic ("treat Node-core imports as resolved when an
`@types/node` package exists on disk") was prototyped and **rejected**: tsc does
*not* resolve `node:*` from an on-disk `@types/node` alone (with `types` absent or
`types: []`, a minimal project still reports `TS2591`), and the heuristic
regressed `node-protocol-no-node-types-basic` by suppressing two real `TS2591`s
it picked up from the repo-root `node_modules/@types/node`. The faithful fix is
transitive `/// <reference types="..." />` loading from dependency declaration
files, tracked as future work; full Node/`@types` resolution parity stays out of
scope.

## v0.84 Real-Project Audit

The old `trpc` baseline is retired as the active real-project target.
`auth-kit` is the finite baseline for this phase.

Primary local path: `AUTH_KIT_PROJECT`

Secondary local path: `../../typescript/auth-project/auth-kit`

Fallback local path: `.local-projects/auth-kit`

This workspace measured auth-kit through the secondary local path above:
`/Users/returnf4lse/Desktop/Workspace/typescript/auth-project/auth-kit`.

Preflight commands for future reruns:

- `cargo fmt --check`
- `cargo test`
- `pnpm run oracle:test`
- `pnpm run bench:test`
- `cargo run -q -p surge-ts-cli -- --project /Users/returnf4lse/Desktop/Workspace/typescript/auth-project/auth-kit/tsconfig.json --showConfig`
- `cargo run -q -p surge-ts-cli -- --project /Users/returnf4lse/Desktop/Workspace/typescript/auth-project/auth-kit/tsconfig.json --compatReport --maxDiagnostics 200`
- `pnpm run oracle:compare -- --project /Users/returnf4lse/Desktop/Workspace/typescript/auth-project/auth-kit/tsconfig.json --maxDiagnostics 200`

Measured real-project state for `/Users/returnf4lse/Desktop/Workspace/typescript/auth-project/auth-kit/tsconfig.json`:

| Metric | Value |
| --- | ---: |
| TypeScript diagnostics | 0 |
| surge-ts diagnostics, raw oracle compare | 0 |
| surge-ts diagnostics, compat-report JSON | 0 |
| loaded files total | 65 |
| root source files | 65 |
| root declarations | 0 |
| dependency declarations | 35 |
| generated files | 231 |
| diagnostics from dependency declarations | 0 |
| Rust-only `surge::*` diagnostics in `tsc` profile | 20 |

auth-kit currently matches TypeScript with 0 diagnostics under the measured
command set.

The compat-report and oracle compare surfaces are raw measurements, not
semantic diagnosis. Missing features are fixed in checker, resolver, and
type-model phases rather than in the report layer. They do not invent root-cause
buckets or dependency-noise labels. Root-cause analysis belongs in implementation
notes and targeted fixtures, not in the report layer. v0.84.8 adds real-source
syntax/scope reconciliation fixtures to narrow the gap between toy fixtures and
auth-kit output.

v0.85 adds a generated default-lib foundation. It does not load the full official TypeScript lib files at runtime; instead it generates a small supported subset from the local TypeScript package and loads those generated declarations as ambient default libs. `noLib: true` disables the generated default libs. Full lib.d.ts parity, Node discovery, and `@types` discovery remain future work.

v0.86 keeps auth-kit exact at 0 diagnostics and shifts the hot path away from repeated module-resolution scans. The checker now reuses canonical file identity lookup for module binding instead of repeatedly scanning loaded file lists, and the timing output has been split into nested module-binding and declaration-collection buckets so the remaining work is visible. On the measured auth-kit project, `module_binding` dropped from 22.731s to 2.049s and `type_declaration_collection` dropped from 11.041s to 3.743s, with `ts-rust` benchmark medians improving from 29.34s to 7.42s at `jobs=1` and from 28.47s to 6.20s at `jobs=4`.

v0.87 keeps auth-kit exact at 0 diagnostics and removes another repeated pass: preliminary module type declarations are now reused by the final module-analysis phase instead of being re-collected, which avoids redoing the same declaration lowering for each file. The remaining hot path is still declaration collection and merging, but the measured auth-kit medians improved further to 6.30s at `jobs=1` and 5.67s at `jobs=4`. The timing output now shows `type_declaration_collection` at 3.307s with `module_analysis_collection` at 2.894s, and `module_binding` at 1.835s with `import_binding_resolution` at 294ms.

v0.88 keeps auth-kit exact at 0 diagnostics and adds hard counters around the declaration path so the remaining structural cost is visible instead of inferred. The auth-kit bench medians moved to 6.22s at `jobs=1` and 5.51s at `jobs=4`; the timing dump now shows `type_declaration_collection` at 4.839s with `module_analysis_collection` at 2.824s, `declaration_table_merging_cloning` at 752ms, and `module_binding` at 1.758s with `import_binding_resolution` at 286ms. The counters show 650 module-analysis calls with 0 duplicates, 3,909 table clones, 2,927 merges, 64 module-scope cache hits, and 0 misses. The target is still not met, and declaration collection plus table cloning/merging remains the next structural bottleneck.

v0.89 keeps auth-kit exact at 0 diagnostics and lands the layered type-declaration scope refactor instead of adding more instrumentation. The auth-kit bench medians moved from 6.22s to 2.84s at `jobs=1` and from 5.51s to 2.34s at `jobs=4`. The timing dump now shows `type_declaration_collection` at 1.199s with `module_analysis_collection` at 366.672ms, `declaration_table_merging_cloning` at 2.535ms, `module_binding` at 496.170ms, and `import_binding_resolution` at 316.942ms. The counters dropped to 4 table clones, 327 merges, 1,629 cloned entries, 863 merged entries, 0 generated-default-lib table clones, 0 dependency-declaration table clones, and a `declaration_lookup_layer_count_avg` of 1.14, which confirms layered lookup is actually being exercised. The remaining bottleneck is no longer the big ambient/default/dependency materialization path; the next visible cost is per-file statement checking plus the still-measurable import-resolution and validation work.

v0.90 keeps auth-kit exact at 0 diagnostics and trims a little more overhead from the statement-checking hot path by caching merged scope visibility and reducing repeated symbol-table rebuilding. On the measured auth-kit project, the benchmark medians moved to 2.76s at `jobs=1` and 2.26s at `jobs=4`. The timing dump now shows `type_declaration_collection` at 1.204s with `module_analysis_collection` at 366.826ms, `declaration_table_merging_cloning` at 2.728ms, `module_binding` at 499.466ms, `import_binding_resolution` at 320.765ms, and `per_file_statement_checking` at 678.533ms. The nested statement buckets show `function_declaration_checking` at 637.294ms, `flow_narrowing` at 628.468ms, `variable_declaration_checking` at 242.589ms, `return_statement_checking` at 156.014ms, `object_literal_checking` at 150.116ms, `assignability_checking` at 10.586ms, and `call_expression_checking` at 2.832ms. The counters now show 650 module-analysis calls, 23,734 declaration lookups with a `declaration_lookup_layer_count_avg` of 1.14, 1,963 expression checks, 1,840 expression inferences, 380 property lookups, 136 call resolutions, 158 object-literal property checks, 333 function-body checks, and 772 type clones. The regression risk remains low because the oracle compare still reports exact matches, but the remaining bottleneck is still the function-body/flow path rather than declaration materialization.

v0.91 keeps auth-kit exact at 0 diagnostics and targets the flow-checking hot path directly. The checker now skips flow-state construction for functions that have no flow-relevant locals, avoids expression-flow walks when nothing is tracked, and measures the flow path with dedicated counters so the remaining cost is visible instead of inferred. On the measured auth-kit project, the benchmark medians moved to 2.72s at `jobs=1` and 2.22s at `jobs=4`. The timing dump now shows `type_declaration_collection` at 1.195s with `module_analysis_collection` at 363.038ms, `declaration_table_merging_cloning` at 2.649ms, `module_binding` at 508.418ms, `import_binding_resolution` at 319.178ms, and `per_file_statement_checking` at 665.498ms. The nested statement buckets show `function_declaration_checking` at 625.309ms, `flow_narrowing` at 612.869ms, `variable_declaration_checking` at 237.916ms, `return_statement_checking` at 149.474ms, `object_literal_checking` at 143.692ms, `assignability_checking` at 10.316ms, and `call_expression_checking` at 2.851ms. The flow counters now show `flow_function_count=333`, `flow_function_skipped_count=41`, `flow_statement_count=678`, `flow_expression_visit_count=1806`, `flow_identifier_read_count=759`, `flow_scope_push_count=78`, `flow_scope_pop_count=78`, `flow_future_declaration_collection_count=292`, `flow_future_declaration_entries_total=235`, `flow_state_clone_count=616`, `flow_scope_locals_clone_count=2347`, `flow_branch_merge_count=123`, `flow_branch_merge_scope_count=154`, `flow_read_lookup_count=759`, `flow_read_lookup_scope_steps_total=850`, `flow_return_analysis_walk_count=505`, and `flow_truthiness_check_count=122`. Exact diagnostics stayed stable at 0, raw oracle match stayed yes, and compatReport diagnosticsTotal stayed 0. The remaining bottleneck is still the function-body/flow path, with import resolution still measurable.

v0.92 keeps auth-kit exact at 0 diagnostics and replaces the branch-state clone pattern with a branch snapshot/delta merge path in flow checking. On the measured auth-kit project, the benchmark medians held at 2.720818041999999s at `jobs=1` and 2.2235199169999977s at `jobs=4`, so the wall-clock effect is neutral for now even though the clone counters fell sharply. The timing dump now shows `type_declaration_collection` at 1.205879s with `module_analysis_collection` at 373.910ms, `declaration_table_merging_cloning` at 2.795ms, `module_binding` at 514.167ms, `import_binding_resolution` at 317.343ms, and `per_file_statement_checking` at 693.565ms. The nested statement buckets show `function_declaration_checking` at 648.517ms, `flow_narrowing` at 626.458ms, `variable_declaration_checking` at 246.552ms, `return_statement_checking` at 161.171ms, `object_literal_checking` at 155.031ms, `assignability_checking` at 9.880ms, and `call_expression_checking` at 2.780ms. The flow counters now show `flow_function_count=333`, `flow_function_skipped_count=41`, `flow_statement_count=678`, `flow_expression_visit_count=1806`, `flow_identifier_read_count=759`, `flow_scope_push_count=78`, `flow_scope_pop_count=78`, `flow_future_declaration_collection_count=292`, `flow_future_declaration_entries_total=235`, `flow_state_clone_count=0`, `flow_scope_locals_clone_count=0`, `flow_state_full_clone_avoided_count=370`, `flow_branch_merge_count=123`, `flow_branch_merge_scope_count=154`, `flow_branch_merge_local_iteration_count=22`, `flow_branch_merge_fast_path_count=120`, `flow_branch_empty_delta_count=135`, `flow_branch_changed_local_count=235`, `flow_read_lookup_count=759`, `flow_read_lookup_scope_steps_total=850`, `flow_return_analysis_walk_count=547`, and `flow_truthiness_check_count=122`. The hot-path clone counters now also show `type_clone_count=772`, `object_type_clone_count=278`, `union_type_clone_count=98`, `symbol_name_clone_count=1329049`, `string_key_clone_count=143234`, `flow_local_name_clone_count=711`, `type_name_lookup_string_count=12502`, `string_path_lookup_count=30503`, and `canonical_file_id_lookup_count=14574`. Exact diagnostics stayed at 0, raw oracle match stayed yes, and compatReport diagnosticsTotal stayed 0. The remaining bottleneck is still the function-body/flow path, with import resolution and return-flow walks still measurable. The arena/ID preflight now points to `FileId`/`ModuleId`/`SymbolId` interning first, before any broader `TypeArena` spike.

v0.68.1 hardens the diagnostic coverage metadata, ensuring that `support = "emitted"` accurately reflects current checker capabilities and is backed by testing.

v0.77.1 implements non-null assertions and a parser-safe `as const` foundation under the default `tsc` diagnostic profile. Literal types and tuple constraints are preserved on primitive literals and object/array properties for `as const` expressions. `satisfies` with `as const` behaves correctly. Optional chaining AST evaluation now correctly propagates the `undefined` short-circuit across subsequent non-null assertions (e.g. `a?.b!.c` evaluates to `C | undefined`).
v0.74.1 supports nested optional property/call chains in a conservative way, and optional element access for arrays and tuples. Every optional chain segment still widens the result with `undefined`. `??` removes `undefined` only in the supported subset. `null`-accurate semantics and control-flow narrowing remain unsupported. `ignoreDeprecations` is not used in committed fixtures because TS 7-oriented compatibility should not hide deprecated option behavior.
v0.70 supports package declaration subpath entrypoints.
v0.69 supports narrow bare package declaration entrypoints.
v0.69.1 hardens/refactors this support. v0.72/v0.72.1 used synthetic built-ins, not physical `lib.d.ts` (since superseded by physical-lib loading). `Array<T>` and `ReadonlyArray<T>` are modeled enough to preserve element diagnostics. v0.81 adds narrow synthetic lowering for `Record`, `Partial`, `Pick`, and `Omit` on top of the mapped-type foundation introduced in v0.80.1. This is still not full utility-type support: `Required`, `Readonly`, `ReturnType`, `Parameters`, `Awaited`, and conditional-type-backed utilities remain unsupported or synthetic noise reducers. Full index signatures remain unsupported, while any narrow `Record<string, T>` / string-index fallback stays limited to oracle-backed narrow paths when the implementation explicitly supports it. Standard/DOM globals now come from the physical `lib*.d.ts` graph loaded by default (the generated subset, which v0.85 introduced, is the fallback); full `lib.d.ts` parity and automatic Node/`@types` discovery remain future work. `noLib: true` disables both the physical and generated default libs, keeping standard/DOM globals unavailable.
Supported (declaration resolution): types, typings, index.d.ts, bare scoped/unscoped packages, exact declaration subpaths, exact `exports["."].types` / `exports["./x"].types` declaration targets, physical `lib*.d.ts` loading by default, and standard/DOM globals sourced from those loaded libs.
Out of scope (declaration resolution): exports runtime conditions, main, wildcard exports, automatic `@types` discovery, baseUrl resolution, JS runtime entrypoints, rootDirs, and project references. (`typesVersions` resolution later became supported; see the current state section above.)

The Node tooling is dev-only. Rust crates do not depend on Node tooling, and
`cargo test` does not require `pnpm install`.

## Local workflow

- Do not commit third-party project source.
- Put disposable real-project experiments under `.local-projects/`.
- Keep local copies out of committed tests and fixtures.
- Keep the root TypeScript version pinned intentionally; changing it may shift
  oracle output and should be done on purpose, not by accident.

Example:

```bash
mkdir -p .local-projects
cargo run -p surge-ts-cli -- --project .local-projects/<project>/tsconfig.json --compatReport --maxDiagnostics 200
pnpm run oracle:compare -- --project .local-projects/<project>/tsconfig.json --maxDiagnostics 200
pnpm run oracle:compare -- --file examples/basic.ts
pnpm run oracle:compare -- --file examples/basic.ts --ignoreConfig
```

## What the report tells you

The compatibility report is raw measurement. It helps count the observed
surface without making semantic diagnosis claims:

1. Parser errors
2. Unsupported module syntax
3. Non-relative package imports and side-effect import diagnostics
4. Missing global/lib symbols or unsupported generic syntax
5. Plain type mismatches

The report does not guarantee that a project is expected to pass.
The oracle comparison is also raw measurement. It does not guarantee that
message text or exact spans match; it starts with code, file, and line/column
normalization first.
Diagnostic codes and messages are catalog-driven in `surge-ts-diagnostics`,
so catalog updates can legitimately move oracle output even when checker
semantics stay the same.
Use `--project` for `tsconfig.json`-based projects and `--file` for single
source files. Passing a `.ts` file to `--project` is rejected now so TypeScript
does not misread the file as a config input.

## Current baseline

The current baseline still intentionally avoids:

- full runtime/JS package resolution parity (the declaration side now resolves conditional and pattern `exports`, the `imports` field, `typesVersions`, package self-name, and subpaths)
- explicit `paths` aliases and declaration-only package entries share the same internal resolved module map
- `baseUrl` resolution remains unsupported/deprecated
- full upstream `lib.d.ts` parity (the physical `lib*.d.ts` graph from the local `typescript` package loads by default; the generated subset is the fallback when that package is absent)
- full declaration-file semantics (a narrow declaration-merging, module-augmentation, and `declare class` slice is supported)
- full automatic `@types` discovery (configured `compilerOptions.types` / `typeRoots` packages are supported)
- project references
- incremental or watch behavior
- narrow generic call-site inference exists for simple direct calls, repeated-parameter calls, and array-element calls, but full generic inference, generic classes, overload inference, callback contextual inference, higher-order inference, constraint enforcement, and tuple-valued implicit generic returns remain unsupported
- enums and namespaces
- CommonJS or bundler semantics
- generic constraints enforcement
- mixed default + named imports
- v0.81 only lowers `Record`, `Partial`, `Pick`, and `Omit` in a narrow synthetic path; the rest of the utility-type ecosystem remains out of scope

The current declaration and diagnostic baseline includes:

- exact ambient `declare module "pkg"` blocks are supported
- ambient modules resolve before package stubbing
- bare package imports (e.g. `pkg` or `@scope/pkg`) and exact subpaths resolve to declaration entrypoints (`types`, `typings`, `exports["types"]`, or `index.d.ts` fallback) in project mode
- resolved package `.d.ts` files act as external modules and do not leak private helpers globally
- default import, namespace import, and re-export behavior for ambient modules and package entrypoints is pinned
- duplicate `interface` declarations merge across files, reopened ambient modules, and module augmentations; a conflicting property reports TS2717 and the first declaration wins, while duplicate ambient `var`/`const`/`function` globals stay pinned
- unsupported declaration syntax remains parser-safe and emits stable diagnostics
- TS2882 is catalog-backed and is emitted for unresolved side-effect imports such as `import "reflect-metadata";`
- ordinary missing package imports still produce TS2307 by default
- `--stubExternalModules` suppresses non-relative missing-module diagnostics, including the side-effect TS2882 form, while leaving relative missing modules and resolved package declaration errors unchanged
- full runtime/JS package resolution, full automatic `@types` discovery, and full `lib.d.ts` parity are still out of scope
- explicit type arguments still instantiate generic aliases/interfaces and the narrow generic call-site path still applies them when present
- tuple-valued implicit generic returns are suppressed for now; explicit type-argument substitution still preserves tuple returns

The oracle harness also stays away from those areas. It only measures the
current surface against TypeScript diagnostics; it does not add new resolver or
type-system behavior to make the numbers line up.
File mode is intentionally narrow: it only accepts `.ts` source files for now,
and it is a quick standalone oracle rather than the main compatibility path.

The next phase should still be chosen from oracle and compat-report output, not
from a fixed feature wish list. Module syntax expansion, package import
stubbing, declaration-file ingestion, ambient declaration hardening, physical
`lib*.d.ts` loading by default (with standard/DOM globals sourced from it), and
the diagnostic catalog/codegen foundation are implemented. Current likely
blockers are common expression syntax, automatic `@types` discovery, React/JSX
type semantics, lib overload resolution, and the remaining deeper `lib.d.ts`
type semantics.

## Note on Type Assertions (v0.73)

Type assertions (`as` expressions) were chosen for v0.73 because they are extremely common in real TypeScript projects, particularly around parsed data, library boundaries, and compatibility shims. By implementing a narrow parsing and inference surface for primitive assertions, aliases, and built-in arrays, we significantly reduce false-positive TS2322 cascades without needing full TypeScript assertion semantics. Dominant blockers remaining after this phase continue to revolve around ambient `@types` package discovery, missing DOM/Node globals, and `lib.d.ts` semantics which often surface as TS2304 errors.

## Note on Optional Chaining and Nullish Coalescing (v0.74/v0.74.1)

v0.74.1 supports nested optional property/call chains in a conservative way, and optional element access for arrays and tuples. Every optional chain segment still widens the result with `undefined`. `??` removes `undefined` only in the supported subset. `null`-accurate semantics, full control-flow narrowing, `??=`, and non-null assertions remain unsupported.

## Note on Benchmark Harness (v0.75/v0.75.2)

v0.75/v0.75.2 adds a compiler speed benchmark harness (`scripts/bench/compare-compilers.ts`) along with diagnostic-drift-aware reporting. This is a developer-facing regression tool comparing no-emit project checks across `tsc`, `tsgo` (optional), and the `surge-ts-cli` release binary. It enforces a TS 7-oriented policy that avoids `ignoreDeprecations` in committed fixtures and requires looking at semantic equivalence alongside wall-clock performance. These are local-machine-relative developer aids; SVG/HTML reports are visualization aids, not marketing claims. Diagnostic drift must be read with timing.

## Note on Type Operators (v0.78)

v0.78 implements a parser-safe foundation for `typeof value`, `keyof T`, and the `keyof typeof constObject` pattern, in a narrow type-position subset. The `typeof` type query resolves top-level or in-scope values to their inferred types. `keyof` resolves object and interface types to string literal unions of their properties. If a value or type is unresolved or unsupported, `surge-ts` defaults to parser-safe conservative emission, outputting `TS2304` or resolving to `Unknown` to match TypeScript's fallback behavior. Advanced types like `typeof import("pkg")`, namespace/class constructor `typeof`, conditional types, template literal types, index signatures, and exact intersection-of-keys semantics for unions remain unsupported.

## Note on Indexed Access Types (v0.79/v0.79.2)

v0.79 implements a parser-safe indexed access type foundation (`T[K]`, `T[keyof T]`). It supports narrow indexed access types including object/interface string-literal property lookup, `T[keyof T]` value unions, and tuple numeric literal indexing. v0.79.2 fixes unresolved-key indexed access diagnostic parity and non-null assertion optional chain parity, ensuring that the default `tsc` profile emits `TS2304` and `TS2538` cascades correctly, and that optional chain `undefined` propagation behaves accurately around non-null assertions and `satisfies` expressions, matching TypeScript's cascading behavior. Advanced usages like conditional types, template literal types, index signatures, and generic indexed access remain unsupported at that historical point. v1.1 later adds a narrow concrete-substitution slice for `T["key"]`, `T[K]`, and `T[keyof T]`, so this note should be read as pre-v1.1 context only.

## Note on Mapped Types (v0.80.1)

v0.80.1 supports a narrow mapped type foundation.
Supported: `{ [K in keyof T]: T[K] }` and `{ [K in keyof T]?: T[K] }` over concrete object/interface inputs after explicit generic substitution.
Unsupported: key remapping, conditional types, template literal types, index signatures, readonly mapped semantics, modifier arithmetic, generic inference, `@types`, physical `lib.d.ts`, DOM/Node globals.
Utility types are not automatically "full TypeScript utility types" just because mapped types exist. If `Partial`, `Record`, `Pick`, `Omit` remain synthetic aliases/noise reducers, say so clearly.
