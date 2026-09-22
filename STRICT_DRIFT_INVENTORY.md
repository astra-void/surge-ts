# Strict Drift Inventory

Inventory of the non-gating message-text and span/column drift in the oracle
preset sweep. **As of 2026-09-22 (`7ea0cddb`) 17 of 387 registered presets
drift** — 16 on message text, 2 on column, one on both. None of them fails the
normal gate. The 2026-09-03 snapshot, when there was no drift at all, is kept
below as history together with the worked examples of what closed each class.

Drift, when it exists, is confined to the column and to the message text at an
already-correct `(file, code, line)`: every entry recorded here still matched
**code-count and file/code/line** under the normal gate. Drift never implies a
missing, extra, mis-filed, or mis-lined diagnostic.

> **Document structure.** § "Current snapshot (2026-09-22)" below is the only
> section that describes the present state. § "Historical snapshot
> (2026-09-03)" and the drift tables under it record deltas that no longer
> reproduce. §§ 1–11 are a **dated historical log** of earlier
> sweeps (75-preset and 78-preset registries) and the passes that closed them;
> their counts, tables, and "remaining" lists do **not** describe current
> behavior. The canonical current-state summary is
> [CURRENT_STATUS.md](CURRENT_STATUS.md).

## Current snapshot (2026-09-22)

- Commit: `7ea0cddb` (release CLI built in a detached worktree at that commit
  and passed to the sweep with `SURGE_TS_BIN`). The sweep ran from the primary
  working tree, where exactly one preset, `block-arrow-return-basic`, carried
  an uncommitted edit; that preset, and every per-location delta in the tables
  below, come from fixtures extracted with `git archive 7ea0cddb`.
- TypeScript oracle: 7.0.2 (pinned)
- Scope: all **387** registered oracle presets, `--maxDiagnostics 200`

| Run | Command flags | Result |
| --- | --- | --- |
| Normal gate | (none) | **387 PASS / 0 FAIL** |
| Strict messages | `--strictMessages` | **371 PASS / 16 FAIL** |
| Strict spans | `--strictSpans` | **385 PASS / 2 FAIL** |
| Both | `--strictMessages --strictSpans` | **370 PASS / 17 FAIL** |

Every preset below matches `tsc` at code-count and file/code/line; the delta is
the message text (or column) at that location.

**Message-text drift (16 presets):**

| Preset | Location | `tsc` says | surge says |
| --- | --- | --- | --- |
| `arithmetic-operand-rules-basic` | `index.ts:13:18` TS2365 | `'bigint' and '1n'` | `'bigint' and 'bigint'` |
| `arrow-unit-return-widening-basic` | `index.ts:11:7` TS2322 | `'"a" \| "b"'` | `'ReturnType<(c: boolean) => "a" \| "b">'` |
| `block-arrow-return-basic` | `index.ts:28:14` TS2322 | `'{ user: { name: string; } \| null; }'` | `'{ user: any; }'` |
| `conditional-expression-mismatch-anchor-basic` | `index.ts:15:7` TS2322 | `'{ p: string; } \| { p: number; }'` | `'{ p: "s"; } \| { p: number; }'` |
| `discriminant-exhaustion-never-basic` | `index.ts:56:13` TS2322 | `'{ kind: "line"; length: number; }'` | `'{ kind: string; length: number; }'` |
| `filter-inferred-predicate-basic` | `index.ts:11:47` TS2339 | `type 'Event'` | `type '{ type: "started"; }'` |
| `function-literal-alias-return-inference-basic` | `index.ts:19:14` TS2322 | `'object[]'` | `'any[]'` |
| `global-this-member-and-inference-basic` | `global-this.ts:9:9` TS2322 | the expanded `fetch` signature | `'Fetch'` |
| `keyof-union-and-mapped-distribution-basic` | `index.ts:13:7`, `:21:7` TS2322 | `'"kind" \| "shared"'`, `'"x" \| "y"'` | `'ShapeKey'`, `'WithIndex'` |
| `lookup-error-type-propagation-basic` | `index.ts:14:17`, `:17:21` TS2339 | `'{ $on(): void; }'` | `'{ $on: () => void; }'` |
| `member-write-missing-property-basic` | `index.ts:25:13` TS2339 | `'() => void'` | `'() => unknown'` |
| `optional-discriminant-narrowing-basic` | `index.ts:32:9` TS2322 | `'Mixed'` | the expanded union |
| `recursive-mapped-alias-member-basic` | `index.ts:30:14`, `:31:14` TS2322 | `'number'`; `typeof import("…").$input` | `'Replace'`; `'typeof $input'` |
| `rest-tuple-literal-context-basic` | `index.ts:17:51`, `:20:14` TS2322 | `'string'`; `'[true]'` | `'"x"'`; `'[boolean]'` |
| `tuple-literal-length-anchor-basic` | six sites, e.g. `index.ts:8:14` TS2322 | `'[number, (string \| undefined)?]'`; `'number'`; `'[number, string, true]'` | `'[number, string \| undefined]'`; `'2'`; `'[number, string, boolean]'` |
| `type-only-namespace-export-type-query-basic` | `consumer.ts:7:14` TS2322 | `'(id: number) => boolean'` | `'Create'` |

**Column drift (2 presets):** `block-arrow-return-basic` (one of its six
diagnostics lands on a different column) and
`parser-classified-diagnostics-basic` (15 of 20 locations match on column).

The classes, roughly: an alias name where tsc prints the expansion or the
reverse (`ShapeKey`, `Fetch`, `Create`, `Mixed`, `ReturnType<…>`); a fresh
literal left unwidened, or widened where tsc keeps it, in the rendered source
type (`'2'`, `[boolean]`, `{ p: "s" }`, `{ kind: string }`); a member degraded
to `any`/`unknown` (`{ user: any }`, `() => unknown`, `any[]`); and method-vs-
property rendering (`$on(): void`). None has been triaged further yet.

## Historical snapshot (2026-09-03)

> **Historical.** This snapshot does not describe current behavior; see
> § Current snapshot (2026-09-22) above.

- Commit: `b090760` (clean checkout; built and run from a detached worktree)
- TypeScript oracle: 7.0.2 (pinned)
- Scope: all **122** registered oracle presets, `--maxDiagnostics 200`

| Run | Command flags | Result |
| --- | --- | --- |
| Normal gate | (none) | **122 PASS / 0 FAIL** |
| Strict messages | `--strictMessages` | **122 PASS / 0 FAIL** |
| Strict spans | `--strictSpans` | **122 PASS / 0 FAIL** |
| Both | `--strictMessages --strictSpans` | **122 PASS / 0 FAIL** |

**There is no strict drift left on the registered presets.** Across all 122 the
sweep saw 206 `tsc` diagnostics and 206 `surge-ts` diagnostics with
`onlyTsc = 0`, `onlyRust = 0`, `messageDriftOnly = 0` and `spanDriftOnly = 0`.

This is a statement about these fixtures, not about arbitrary code, and the
strict flags stay **non-gating**: a preset added tomorrow may record a drift
without failing the normal gate, which is the point of keeping the dimensions
separate.

### What closed the nine drifts (2026-09-03)

The eight message-text targets and one span target inventoried below were all
closed at this commit. The tables are kept because they are the worked examples
— each row names the exact delta the fix had to erase.

**The rule that made it safe: display data belongs on the type *handle* or in
the diagnostic layer, never on the interned payload or a `TypeReference`'s
`display`.** `store.rs` compares `left.display == right.display` when deciding
whether one interned payload may substitute another, so changing a display
string changes which cached expansion is reused — and that changes diagnostics.
Two attempts that edited display strings directly were measured and reverted
(one cost 2 trpc false positives, one cost 18 across ky/unnamed/trpc).

What worked, per class:

1. **Function-type parameter names and type-parameter heads** — carried on
   `FunctionType`, not `FunctionTypePayload`, so signatures differing only in
   parameter names still share one canonical payload. `name()` memoizes into the
   *shared* payload, so a handle-local rendering must bypass the memo.
2. **Alias names for unions and signatures** — the same handle-local trick on
   `UnionType`/`FunctionType`. `type Level = "a" | "b"` renders as `Level` while
   the interned member list is untouched, so nothing that inspects
   `Type::Union` had to learn to peel.
3. **Alias vs expansion, in both directions** — a generic instantiation renders
   from its *resolved* arguments (`Dispatch<SetStateAction<number>>`), and a
   conditional alias renders as the branch it resolved to via a handle flag on
   `TypeReference` that redirects rendering while leaving `id`, `display` and
   every sharing decision byte-identical.
4. **Same-named types from different declarations** — the diagnostic layer
   qualifies the non-local side (`import("/abs/path/store").Store`) when the two
   rendered names would collide, recovering the declaring file from the nominal
   id both `ObjectType.alias_id` and `TypeReference.id` already carry.
5. **Enum nominal names** — a literal has no handle, so the *alias* resolution
   wraps an enum-lowered type in a nominal reference whose display carries the
   enum. tsc qualifies an exported enum and names a file-local one bare; a
   single member displays as the enum, not `Enum.Member`.
6. **Optional-property union rendering** — the object-literal mismatch names the
   property's written type, not the optionality-widened `T | undefined`, which
   is what tsc's elaboration reports.
7. **The span target** — a contextually-typed arrow's return mismatch is now
   reported as one whole-signature TS2322 on the assignment, rendered from the
   widened union of the types the body actually returned. A block-body arrow
   never derived its return type from the body, which is why an earlier attempt
   at this was inert.

Nominal enum *assignability* was tried and reverted; see
`tests/compat-projects/nominal-enum-display-basic/README.md`.

### Historical: message-text drift (8 targets, span matched) — closed 2026-09-03

| Target | Loc | tsc message | surge message |
| --- | --- | --- | --- |
| `namespace-nested-member-lazy-scope-basic` | `src/index.ts:5:7` TS2322 | …type `'(value: string) => void'` | …type `'(string) => void'` |
| `function-type-binding-pattern-param-basic` | `src/index.ts:8:7` TS2322 | `'<T extends FieldBag = FieldBag>(props: CtrlProps<T>) => string'` | `'(CtrlProps<T>) => string'` |
| `interface-extends-call-signature-basic` | `src/index.ts:15:7` TS2322 | …type `'{ a: string; }'` | …type `'PropsOf<Exotic<{ a: string; }>>'` |
| `query-generics-observer-basic` | `src/index.ts:34:3` TS2322 | `'(data: string) => void'` → `'(data: number) => void'` | `'(string) => void'` → `'(number) => void \| undefined'` |
| `express-augmentation-cycle-collision-pinned` | `src/b.ts:7:7` TS2322 | `'import("…/node_modules/storekit/store").Store'` → `'Store'` | `'Store'` → `'Store'` |
| `namespace-member-signature-siblings-basic` | `src/index.ts:9:14` TS2322 | `'Dispatch<SetStateAction<number>>'` | `'(number \| (number) => number) => void'` |
| `enum-member-type-basic` | `src/index.ts:13:14`, `24:14` TS2322 | `'import("…/src/index").Color'` / `.Label` | `'number'` / `'string'` |
| `ambient-module-sibling-scope-basic` | `src/index.ts:9:31` TS2345 | parameter of type `'Level'` | parameter of type `'"a" \| "b"'` |

Four recurring classes, all display-level:

1. **Function-type parameter names are dropped.** surge renders
   `(string) => void` where tsc renders `(value: string) => void`, and drops
   type-parameter lists from the signature head. Three of the eight rows.
2. **Alias / nominal name vs. structural expansion, in both directions.**
   Sometimes surge shows the alias where tsc shows the expansion
   (`PropsOf<Exotic<…>>`), sometimes the reverse (`number` where tsc prints the
   enum's nominal `import("…").Color`, `"a" | "b"` where tsc prints `Level`).
3. **Same-named types from different declarations are indistinguishable.** tsc
   disambiguates with an `import("<absolute path>").Name` form; surge prints the
   bare name on both sides, so the message reads `'Store' is not assignable to
   type 'Store'`. This is the honest rendering of a real distinction the
   diagnostic text cannot currently express.
4. **Optional-property union rendering leaks into the target type**
   (`(number) => void | undefined`).

### Historical: span drift (1 target) — closed 2026-09-03

`contextual-return-any-collapse-basic`, 3 of its 6 rows
(`src/index.ts` lines 21, 42, 45, all TS2322). tsc anchors the diagnostic at the
assignment/function position (column 14) and elaborates downward through a
nested "Types of property 'x' are incompatible" chain to the leaf mismatch;
surge anchors directly at the leaf property (columns 47, 86, 106) and reports
only the leaf message. Line, file, and code match on every row; the differing
column also moves these rows out of the exact-location message comparison, so
they never surface as `messageMatch = false`. This is the same bucket-5 pattern
described in § 1 of the historical log below.

### Commands

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200
```

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages
```

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictSpans
```

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages --strictSpans
```

The strict runs exit non-zero by design when drift exists. Pass
`--strictMessages --strictSpans` as two separate arguments; the sweep rejects
them quoted as one.

**Keep the build worktree alive until the measurements are done.** The
generated default-lib subset is resolved through `env!("CARGO_MANIFEST_DIR")`
(`crates/surge-ts-checker/src/default_lib/loader.rs`), so a binary built in a
throwaway worktree stops finding `generated-libs/` the moment that worktree is
removed — even if the binary itself was copied elsewhere first. The failure is
silent apart from a `unknown lib 'es2024.full'` warning on stderr, and it
surfaces as a flood of `TS2304 Cannot find name 'Promise'` on any preset whose
tsconfig does not pin `lib`. Presets that resolve a physical `lib*.d.ts` set are
unaffected, so the damage looks target-specific rather than environmental. Note
that the pinned oracle (`typescript@7.0.2`) ships **no** `lib*.d.ts` files at
all, which is why the generated fallback carries these fixtures.

---

# Historical log

**Everything below is a dated record of earlier sweeps.** The registry was 75
presets at § 1 and 78 by § 10; it is 122 now. Counts, "remaining" lists, and
"stays clean" statements in these sections describe the state at the time of
that pass only.

## 1. Sweep of 2026-06-17 (75 presets)

- Date: 2026-06-17
- TypeScript oracle version: pinned (unchanged)
- Scope: all 75 registered oracle presets, `--maxDiagnostics 200`

### 1.1 Summary

| Run | Command flags | Result | Notes |
| --- | --- | --- | --- |
| Normal gate | (none) | **75 PASS / 0 FAIL** | code-count + file/code/line all match |
| Strict messages | `--strictMessages` | **72 PASS / 3 FAIL** | 3 targets have pure message-text drift |
| Strict spans | `--strictSpans` | **55 PASS / 20 FAIL** | 20 targets have column/span drift |
| Both | `--strictMessages --strictSpans` | **52 PASS / 23 FAIL** | the two sets are disjoint (3 + 20 = 23) |

Both strict flags are supported together; the harness gates each dimension
independently (`sweep-presets.ts` `deriveResult`), so a combined run reproduces
exactly the per-target `messageMatch` / `spanMatch` flags from the two single-flag
runs. The 3 message-drift targets all have matching spans; the 20 span-drift
targets all have matching-or-uncompared messages. No overlap.

**Drift totals (by diagnostic row, not by target):**

- Message-only drift rows: **4** (across 3 targets)
- Span-drift rows: **35** (across 20 targets)
  - of which **19** are a pure column offset with byte-identical message text
  - of which **16** also carry a *co-located* literal-vs-widened message
    difference that the harness cannot surface as `messageMatch=false` because the
    differing column moves the diagnostic out of the exact-location message
    comparison (see bucket 5).
- Gating status: **every** drift row still matches code-count and
  file/code/line under the normal gate. No drift implies a missing, extra,
  mis-filed, or mis-lined diagnostic. Drift is confined to column and message
  text at an already-correct (file, code, line).

## 2. Drift table

Columns: `Target` · `File` · `Code` · `TS line:col` · `RS line:col` · `Drift` ·
`TS message` → `RS message`. All rows are code-count- and file/code/line-matched
under the normal gate (omitted from the table since it is uniformly *yes*).

### 2a. Message-only drift (span matches exactly; fails `--strictMessages`)

| Target | File | Code | Loc | TS message | RS message |
| --- | --- | --- | --- | --- | --- |
| generic-cache-module-source-not-persisted-basic | src/index.ts | TS2353 | 4:27 | …does not exist in type `'Box<string>'`. | …does not exist in type `'{ value: string; }'`. |
| generic-cache-module-source-not-persisted-basic | src/index.ts | TS2353 | 5:27 | …does not exist in type `'Box<string>'`. | …does not exist in type `'{ item: string; }'`. |
| jsx-intrinsic-elements-basic | src/index.tsx | TS2322 | 3:25 | …assignable to type `'{ disabled?: boolean \| undefined; children?: unknown; }'`. | …assignable to type `'{ children?: unknown; disabled?: boolean; }'`. |
| jsx-dom-physical-lib-prop-basic | src/index.tsx | TS2322 | 10:19 | Type `'string'` is not assignable to type `'URL'`. | Type `'string'` is not assignable to type `'{ hash: string; … }'` (full structural expansion — see Detail D1). |

**Detail D1 — jsx-dom-physical-lib-prop-basic full RS message:**

> Type 'string' is not assignable to type '{ hash: string; host: string; hostname: string; href: string; origin: string; password: string; pathname: string; port: string; protocol: string; search: string; searchParams: { append: (string, string) => void; delete: (string, string) => void; entries: () => { [key: string]: any; }; forEach: ((string, string, unknown) => void, any) => void; get: (string) => string | undefined; getAll: (string) => string[]; has: (string, string) => boolean; keys: () => { [key: string]: any; }; set: (string, string) => void; size: number; sort: () => void; toString: () => string; values: () => { [key: string]: any; }; }; toJSON: () => string; toString: () => string; username: string; }'.

TypeScript prints the alias name `URL`; surge-ts prints the fully expanded
structural object type (and renders method types without parameter names, e.g.
`(string, string) => void`).

### 2b. Span drift (fails `--strictSpans`)

`Drift = span` means the message text is byte-identical and only the column
differs. `Drift = both` means the same row also carries a literal-vs-widened
message difference that is masked from the message comparator by the column
offset.

| Target | File | Code | TS line:col | RS line:col | Drift | Message note |
| --- | --- | --- | --- | --- | --- | --- |
| ambient-module-reopen-merge-basic | src/index.ts | TS2741 | 8:7 | 8:23 | both | `{ id: string; }` → `{ id: "u1"; }` |
| auto-types-ancestor-visibility-basic | src/index.ts | TS2322 | 4:7 | 4:21 | span | identical |
| auto-types-nearest-wins-basic | src/index.ts | TS2322 | 2:7 | 2:21 | span | identical |
| auto-types-node-basic | src/index.ts | TS2322 | 4:7 | 4:21 | span | identical |
| auto-types-scoped-basic | src/index.ts | TS2322 | 2:7 | 2:21 | span | identical |
| declaration-reexports-hardening | src/index.ts | TS2322 | 18:7 | 18:23 | span | identical |
| declarations-basic | src/index.ts | TS2322 | 6:5 | 6:17 | both | `number` → `123` |
| declarations-basic | src/index.ts | TS2322 | 9:23 | 9:29 | both | `number` → `123` |
| declarations-basic | src/index.ts | TS2322 | 13:5 | 13:23 | span | identical |
| declare-global-interface-basic | src/index.ts | TS2741 | 5:7 | 5:35 | span | identical |
| declare-global-interface-basic | src/index.ts | TS2322 | 8:7 | 8:25 | span | identical |
| declare-global-window-physical-lib-basic | src/index.ts | TS2322 | 6:7 | 6:21 | span | identical |
| diagnostics-pack | src/assignability.ts | TS2322 | 6:5 | 6:26 | both | `number` → `1` |
| diagnostics-pack | src/assignability.ts | TS2741 | 9:5 | 9:17 | both | `{ name: string; }` → `{ name: "Alice"; }` |
| diagnostics-pack | src/calls.ts | TS2554 | 6:12 | 6:1 | span | identical (TS anchors excess arg, RS anchors call) |
| diagnostics-pack | src/functions.ts | TS2355 | 10:23 | 10:10 | span | identical (TS anchors return type, RS anchors fn name) |
| generic-cache-dependency-instantiation-basic | src/a.ts | TS2322 | 4:26 | 4:33 | both | `number` → `123` |
| generic-cache-dependency-instantiation-basic | src/b.ts | TS2322 | 4:26 | 4:33 | both | `string` → `"wrong"` |
| interface-merging-across-files-basic | src/index.ts | TS2741 | 6:7 | 6:22 | both | `{ TOKEN: string; }` → `{ TOKEN: "x"; }` |
| interface-merging-across-files-basic | src/index.ts | TS2322 | 12:3 | 12:9 | span | identical |
| interface-merging-basic | src/index.ts | TS2741 | 14:7 | 14:23 | both | `{ id: string; }` → `{ id: "u1"; }` |
| interface-merging-basic | src/index.ts | TS2322 | 20:3 | 20:9 | both | `number` → `123` |
| interface-method-merge-basic | src/index.ts | TS2322 | 15:7 | 15:21 | span | identical |
| module-augmentation-add-export-basic | src/index.ts | TS2322 | 5:7 | 5:21 | span | identical |
| module-augmentation-package-interface-basic | src/index.ts | TS2741 | 8:7 | 8:25 | both | `{ id: string; }` → `{ id: "c1"; }` |
| module-augmentation-package-interface-basic | src/index.ts | TS2322 | 14:3 | 14:10 | both | `number` → `123` |
| package-declarations | src/missing-export.ts | TS2322 | 6:7 | 6:19 | span | identical |
| package-declarations | src/signatures.ts | TS2322 | 12:23 | 12:29 | both | `number` → `123` |
| package-declarations | src/subpaths.ts | TS2322 | 10:5 | 10:24 | span | identical |
| package-declarations | src/typings.ts | TS2322 | 2:7 | 2:19 | span | identical |
| parallel-ordering-basic | src/a.ts | TS2322 | 2:14 | 2:31 | both | `number` → `123` |
| parallel-ordering-basic | src/c.ts | TS2322 | 6:3 | 6:9 | both | `number` → `123` |
| parallel-ordering-basic | src/index.ts | TS2322 | 5:7 | 5:22 | both | `number` → `42` |
| tsx-jsx-basic | src/index.tsx | TS2322 | 4:7 | 4:21 | span | identical |
| type-roots-basic | src/index.ts | TS2322 | 4:7 | 4:21 | span | identical |

**Span-anchor policy observed (consistent across all 35 rows):** for the same
`(file, code, line)`, TypeScript anchors the diagnostic on the *left/target*
syntactic node and surge-ts anchors on the *right/value* node:

| Code / construct | TypeScript anchor | surge-ts anchor |
| --- | --- | --- |
| TS2322 on `let/const x: T = v` | declaration name `x` | initializer `v` |
| TS2322 on object-literal member `k: v` | property key `k` | value `v` |
| TS2741 missing prop on `x: T = { … }` | declaration name `x` | object literal `{ … }` |
| TS2554 wrong arg count | the excess/missing argument | the call expression start |
| TS2355 missing return | the return-type annotation | the function name |

No row points at a different line, a different file, or a semantically unrelated
node. Every divergence is a different — and individually defensible — anchor on
the **same source line for the same diagnostic**.

## 3. Classification

### Bucket 1 — Safe to ignore for now
The **19 pure span-only rows** (`Drift = span`, message identical): same code, same
line, same diagnostic, only the column anchor differs. No user-facing behavior
risk and no type/checking mismatch implied. Harmless until a strict-span gate is
desired.

### Bucket 2 — Cheap span polish
All 35 span-drift rows are candidates, but they reduce to a small number of
**systematic anchor policies** (the table in §2b). Fixing them most likely means
threading the existing *target/declaration-name* span (TS2322/TS2741 on variable
and property assignments) and the *argument*/*return-type* spans (TS2554/TS2355)
instead of the value/initializer span. Low semantic risk, but note this is a
*systematic* span-emission policy touching the assignability-diagnostic path
broadly, not a per-fixture tweak — out of scope for this inventory task.

### Bucket 3 — Diagnostic construction / display polish
Message differs because of how the *type is displayed*, not because of a different
resolved type:

- **Literal vs widened display** (16 "both" rows): tsc widens the source literal
  for the message (`123`→`number`, `"u1"`→`string`, `{ id: "u1"; }`→`{ id: string; }`),
  surge-ts prints the literal/narrow form. This is the known
  display-widening policy gap; here it remains for TS2322/TS2741 on variable and
  object-literal assignments.
- **Alias vs structural display** (TS2353 `Box<string>` vs `{ value: string; }`;
  TS2322 `URL` vs full expansion): tsc prints the alias/named type, surge-ts
  prints the expanded structural type.
- **Optional-prop / member-order normalization** (jsx-intrinsic-elements:
  `{ disabled?: boolean | undefined; children?: unknown; }` vs
  `{ children?: unknown; disabled?: boolean; }`): member ordering differs and tsc
  renders `boolean | undefined` for an optional where surge-ts renders
  `boolean`.
- **Parameter-name-less function display** (Detail D1): surge-ts prints
  `(string, string) => void` (no parameter names).

All require type-formatting / diagnostic-argument improvements. **Do not fix in
this task.**

### Bucket 4 — Real semantic suspicion
**None found.** No span points at a wrong line or unrelated expression; every span
divergence is a defensible alternate anchor on the correct line. No message drift
implies a different *resolved* type — only a different *display* of the same type
(alias-vs-structural, literal-vs-widened, member-order). Code-count and
file/code/line match everywhere. There is no evidence of incorrect checker
behavior hiding behind the drift.

### Bucket 5 — Harness / reporting observations (not checker behavior)
- **Span drift masks co-located message drift.** `compareMessages` only compares
  messages at an *exactly* matching `(file, code, line, column)`. When the column
  differs (span drift), a real literal-vs-widened message difference on the same
  row is never message-compared, so it does **not** count toward
  `messageDriftOnly`. 16 of the 35 span rows carry such hidden message drift. This
  is why **diagnostics-pack reports `message=yes` (green) under `--strictMessages`
  even though it carries `number`→`1` and `{ name: string; }`→`{ name: "Alice"; }`
  display drift** — both are hidden behind a span offset. diagnostics-pack is
  genuinely green under the normal gate and under `--strictMessages`; it fails only
  `--strictSpans` (4 span rows). This is a comparator scoping limitation, not a
  checker defect and not a reason to change the comparator.
- The sweep summary's `messageDriftOnly` / `spanDriftOnly` counters only count
  targets that *passed* the active gate, so under a combined strict run both read
  `0` while 23 targets fail — expected, not a regression. Per-target
  `messageMatch` / `spanMatch` flags (via `--json`) are the reliable inventory
  source.

## 4. Recommended next tasks

Ranked by value/risk. Each is a *future* targeted task, not part of this inventory.

1. **(P2, low risk) Literal-vs-widened display for TS2322/TS2741 assignment
   diagnostics.** Extend the existing display-widening policy (already applied to
   TS2345/TS2365/TS2367 per project notes) to cover variable- and
   object-literal-initializer assignability messages so `123`→`number` and
   `{ id: "u1"; }`→`{ id: string; }`. Touches diagnostic message construction
   only; resolves 16 message-display rows and is the single highest-count drift
   class. Risk: must not over-widen contexts where tsc *keeps* the literal
   (verify against the existing widening rule). Gate impact: none (non-gating
   today).
2. **(P1, low–moderate risk) Align TS2322/TS2741 span to the assignment target.**
   Anchor variable-initializer and object-literal-member assignability
   diagnostics on the declaration name / property key (matching tsc) instead of
   the value expression. This is the dominant span class (most of the 35 rows).
   Risk: systematic change to span emission on the assignability path; needs a
   focused span sweep (`--strictSpans` on the affected presets) to confirm no
   collateral movement. Value: would clear the majority of span-drift targets.
3. **(P2, moderate risk) Alias-aware type display in messages
   (`Box<string>`/`URL` instead of structural expansion).** Prefer a named/alias
   form when one is in scope at the diagnostic site, and (separately) emit
   parameter names in function-type display. Improves readability of TS2353/TS2322
   messages. Risk: type-formatting changes can ripple across many messages;
   should be done behind an `--strictMessages` sweep with careful review. Lower
   priority because it affects only 3–4 rows.

Not recommended: the TS2554 (excess-arg) and TS2355 (return-type) span anchors are
single rows each and lower value; fold them into task 2 only if convenient.

## 5. Non-goals

This inventory **does not** and **must not**:

- change any checker semantics, type inference, or diagnostic logic;
- change fixtures, expected output, or the diagnostics-pack project;
- weaken, strengthen, or re-scope any oracle gate;
- touch performance/cache/arena code, generated libs, or the TypeScript version;
- convert any non-gating drift into a gating failure;
- apply report-layer normalization to hide drift.

It records the current drift faithfully so that the follow-up tasks in §4 can be
scoped and prioritized.

## 6. Commands run

```bash
# Baseline (normal gate)
pnpm run oracle:sweep -- --all --maxDiagnostics 200
#   → selected 75, passed 75, failed 0; messageDriftOnly 3, spanDriftOnly 20

# Strict messages only
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages
#   → passed 72, failed 3 (exit 1, expected)

# Strict spans only
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictSpans
#   → passed 55, failed 20 (exit 1, expected)

# Both strict flags (supported together)
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages --strictSpans
#   → passed 52, failed 23 (exit 1, expected)

# Per-target detail extraction for the 23 drift targets
pnpm exec tsx scripts/oracle/compare-tsc.ts --project <target> --json --maxDiagnostics 200
```

Per-target `messageParity.mismatches` (exact-location message drift) and the
`details.onlyTypeScript[Rust].rawDiagnosticFingerprints` column comparison (same
logic as `deriveSpanMatch`) were used to populate §2.

## 7. After assignment-span pass

Follow-up to task §4.2: align TS2322/TS2741 spans to tsc's target-side anchor.
**Span emission only — no checker semantics, type inference, diagnostic codes,
fixtures, gates, libs, or TypeScript version changed.** Diagnostic code-count and
file/code/line still match everywhere (normal gate stays **75 PASS / 0 FAIL**).

| Run | Before | After |
| --- | --- | --- |
| Normal gate | 75 PASS / 0 FAIL | **75 PASS / 0 FAIL** |
| `--strictSpans` | 55 PASS / 20 FAIL | **74 PASS / 1 FAIL** |
| `--strictMessages` | 72 PASS / 3 FAIL | **63 PASS / 12 FAIL** |
| both | 52 PASS / 23 FAIL | **63 PASS / 12 FAIL** |

**What changed (span ownership):**

- TS2322 on `let/const x: T = v` → now anchors on the declaration name `x`
  (was the initializer `v`). `crates/.../checks/var.rs`.
- TS2322 on object-literal member `k: v` → now anchors on the property key `k`
  (was the value `v`). `crates/.../checks/expected.rs`.
- TS2741 missing required property on `x: T = { … }` → now anchors on the
  declaration name `x` (was the object literal `{ … }`). Threaded via a new
  `evaluate_expression_with_expected_type_anchored` `target_span` parameter that
  defaults to `None` (current behavior) for every caller except the variable
  declaration. `crates/.../checks/expected.rs`, `var.rs`.

**Targets fixed on spans (19 of 20):** all prior span-drift targets now pass
`--strictSpans` except `diagnostics-pack`. Of these 19:

- **11 are now fully clean** (pass both strict gates — they were pure span-only
  rows with byte-identical messages): `auto-types-ancestor-visibility-basic`,
  `auto-types-nearest-wins-basic`, `auto-types-node-basic`, `auto-types-scoped-basic`,
  `declaration-reexports-hardening`, `declare-global-interface-basic`,
  `declare-global-window-physical-lib-basic`, `interface-method-merge-basic`,
  `module-augmentation-add-export-basic`, `tsx-jsx-basic`, `type-roots-basic`.
- **8 now pass `--strictSpans` but newly fail `--strictMessages`** because aligning
  the column exposes the *co-located literal-vs-widened message drift* the
  comparator previously could not see (the §2b "both" rows / bucket 5 masking):
  `ambient-module-reopen-merge-basic`, `declarations-basic`,
  `generic-cache-dependency-instantiation-basic`,
  `interface-merging-across-files-basic`, `interface-merging-basic`,
  `module-augmentation-package-interface-basic`, `package-declarations`,
  `parallel-ordering-basic`. This is pre-existing display drift becoming visible,
  not new drift — every exposed message is purely `number`→`123` /
  `{ id: "u1"; }`→`{ id: string; }`-style widening (task §4.1, intentionally out
  of scope here).

**Remaining span drift (1 target):** `diagnostics-pack` still fails `--strictSpans`
on the two non-assignment rows left untouched: TS2554 (excess-arg, anchors the
call vs tsc's excess argument) and TS2355 (missing-return, anchors the function
name vs tsc's return-type annotation). TS2355 needs a parser-captured return-type
span (no span on `ParsedType` today), so it is deferred; see §4 "not recommended".

**Remaining message drift (12 targets):** 3 pre-existing (§2a — `Box<string>`/`URL`
alias display, jsx member-order) + 9 newly-visible literal-vs-widened rows above
(8 cleared-on-spans + `diagnostics-pack`). All are display-formatting drift
(task §4.1 / §4.3), unchanged by this pass.

**Commands run:**

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200                                   # 75/75
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictSpans                     # 74/1
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages                  # 63/12
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages --strictSpans    # 63/12
cargo fmt --check && cargo test --workspace && pnpm run oracle:test                             # all green
```

## 8. After strict diagnostic polish bundle

Follow-up to tasks §4.1 (literal-vs-widened display) and §4 "not recommended"
(TS2554 excess-arg span, TS2355 return-type span). **Diagnostic display/span only
— no checker semantics, type inference, diagnostic codes, fixtures, gates, libs,
or TypeScript version changed.** Diagnostic code-count and file/code/line still
match everywhere (normal gate stays **75 PASS / 0 FAIL**).

| Run | Before (after §7) | After |
| --- | --- | --- |
| Normal gate | 75 PASS / 0 FAIL | **75 PASS / 0 FAIL** |
| `--strictMessages` | 63 PASS / 12 FAIL | **72 PASS / 3 FAIL** |
| `--strictSpans` | 74 PASS / 1 FAIL | **75 PASS / 0 FAIL** |
| both | 63 PASS / 12 FAIL | **72 PASS / 3 FAIL** |

**Fixes (display/span only):**

- **Literal-vs-widened display** (clears all 9 literal/widened message targets:
  `declarations-basic`, `generic-cache-dependency-instantiation-basic`,
  `package-declarations`, `parallel-ordering-basic`, `interface-merging-basic`,
  `interface-merging-across-files-basic`, `module-augmentation-package-interface-basic`,
  `ambient-module-reopen-merge-basic`, plus the co-located rows in
  `diagnostics-pack`). The source-side type in TS2322/TS2741 messages now widens
  fresh literals for display (`123`→`number`, `"wrong"`→`string`,
  `{ id: "u1"; }`→`{ id: string; }`) the way tsc does, while the resolved type and
  assignability are unchanged. Implemented by routing the previously-raw
  `inferred_type.name()` source displays through the existing
  `source_display_name(source, target)` helper (which keeps the literal when the
  *target* is literal-like, e.g. `let x: "a" = "b"`), and by widening the
  object-literal source type for the TS2741 missing-property message.
  - `checks/var.rs` (variable-initializer TS2322 source),
    `checks/expected.rs` (object-literal property TS2322 + TS2741 missing-property
    source), `checks/function/body.rs` (return-statement TS2322 source).
- **TS2554 excess-argument span** (`diagnostics-pack` `src/calls.ts`). A too-many-
  arguments error now anchors the excess-argument range (first excess argument
  through the last supplied argument) instead of the call expression, matching
  tsc (`greet("a", "b")` → col 12, the `"b"`). Too-few-argument errors keep the
  call/callee anchor. `checks/call/mod.rs` (`excess_argument_span` helper).
- **TS2355 missing-return span** (`diagnostics-pack` `src/functions.ts`). A
  missing-return error now anchors the return-type annotation
  (`function f(): number {}` → the `number`) instead of the function name, matching
  tsc. Threaded a new `ParsedFunctionDeclaration.return_type_span` (captured from
  the oxc return-type annotation's inner type span) narrowly through the two
  function-declaration check paths to `emit_missing_return_diagnostic`, falling
  back to the name span when absent. `surge-ts-syntax` AST + parser,
  `checks/function/mod.rs`.

**Deferred (3 message-drift targets — alias/JSX/member-order display):**

- `generic-cache-module-source-not-persisted-basic` (TS2353): tsc prints the
  generic-alias target `Box<string>`; surge-ts prints the structural
  `{ value: string; }` / `{ item: string; }`. **Deferred** — requires the generic
  type-alias instantiation to retain the alias name *and* its instantiated type
  arguments on the resulting object type, plus a `Name<Args>` display path. The
  existing `ObjectType.alias_name` only carries a non-generic name; threading type
  arguments through instantiation is type-identity metadata, not a display-only
  change.
- `jsx-dom-physical-lib-prop-basic` (TS2322): tsc prints the physical-lib nominal
  `URL`; surge-ts prints the full structural expansion (and renders
  method types without parameter names, e.g. `(string, string) => void`).
  **Deferred** — same nominal-name-retention gap as above, plus a separate
  function-type display change to emit parameter names.
- `jsx-intrinsic-elements-basic` (TS2322): member ordering
  (`{ disabled?...; children?...}` vs `{ children?...; disabled?...}`) and optional
  rendering (`boolean | undefined` vs `boolean`). **Deferred** — member-order
  normalization and optional-`| undefined` rendering are object-display policy
  changes that touch every structural-object message, out of scope for this
  localized polish.

**Commands run:**

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200                                   # 75/75
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages                  # 72/3
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictSpans                     # 75/75
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages --strictSpans    # 72/3
cargo fmt --check && cargo test --workspace && pnpm run oracle:test                             # all green
```

## 9. After alias-aware message display pass

Follow-up to §8's deferred message-drift rows (§4.3 alias/structural display).
**Diagnostic display only — no checker semantics, type inference, diagnostic
codes, fixtures, gates, libs, or TypeScript version changed.** `alias_name` is
excluded from type equality and no `alias_id` is added on the new paths, so
assignability is unchanged. Normal gate stays **75 PASS / 0 FAIL**; `--strictSpans`
stays **75 PASS / 0 FAIL**.

| Run | Before (after §8) | After |
| --- | --- | --- |
| Normal gate | 75 PASS / 0 FAIL | **75 PASS / 0 FAIL** |
| `--strictMessages` | 72 PASS / 3 FAIL | **74 PASS / 1 FAIL** |
| `--strictSpans` | 75 PASS / 0 FAIL | **75 PASS / 0 FAIL** |
| both | 72 PASS / 3 FAIL | **74 PASS / 1 FAIL** |

**Fixes (display only):**

- **Generic-alias display** (`generic-cache-module-source-not-persisted-basic`,
  TS2353). A generic instantiation's whole-type display now shows its alias form
  `Box<string>` instead of the structural expansion `{ value: string; }`. The
  display name is built from the *syntactic* type arguments (via the existing
  `parsed_type_display`, which resolves nothing — no diagnostic or caching side
  effects — and, like tsc, keeps a type-alias argument by name rather than
  expanding it) and attached as the resolved object's `alias_name`.
  `infer/types/resolve.rs`.
- **Original declared name through import rename.** The same fixture imports
  `Box as ABox`, and `rename_type_declaration` had overwritten the declaration's
  `name` to the local binding, so the message showed `ABox<string>`. Added a
  display-only `declared_name` to `TypeAliasInfo`/`InterfaceInfo` that captures the
  pre-rename name on the first rename; the generic-alias display uses it so the
  message shows the original `Box<string>` (matching tsc). `symbols/type_declarations.rs`,
  `modules/exports.rs`.
- **Nominal display for cyclic library interfaces** (`jsx-dom-physical-lib-prop-basic`,
  TS2322). `URL` (whose `searchParams` cluster is mutually recursive) resolved with
  `had_error`, which had gated off the display `alias_name`, so the message printed
  the full structural expansion. Now the name is kept whenever the object resolved
  to a real (non-empty) shape; only a collapse to an empty object falls back to the
  structural form. `infer/types/resolve.rs`.

**Deferred (1 message-drift target):**

- `jsx-intrinsic-elements-basic` (TS2322): member ordering
  (`{ disabled?...; children?...}` vs `{ children?...; disabled?...}`) and optional
  rendering (`boolean | undefined` vs `boolean`). **Still deferred** — the object's
  properties are stored in an alphabetical `BTreeMap` built through the arena
  allocation path, so preserving declaration order needs a display-order field on
  the core `ObjectType` populated at construction (the arena/body-sharing area is
  off-limits) plus a global optional-`| undefined` rendering change. Both are
  object-display-architecture changes disproportionate to a single fixture.

**Commands run:**

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200                                   # 75/75
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages                  # 74/1
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictSpans                     # 75/75
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages --strictSpans    # 74/1
cargo fmt --check && pnpm run oracle:test                                             # green
# (cargo test --workspace skipped this pass at the user's request — runtime; the
#  changes are display-only and validated through the oracle sweeps.)
```

## 10. After library-scoped named-object peel display pass

Follow-up sweep on the now-78-preset registry (the suite grew from 75 since §9;
the §9 `jsx-intrinsic-elements-basic` member-order row is no longer present). A
fresh sweep surfaced exactly two remaining `--strictMessages` rows, both the same
class as §9 (§4.3 alias/structural display) but on a *peeled* object that the §9
eager-path fix did not reach. **Diagnostic display only — no checker semantics,
type inference, diagnostic codes, fixtures, gates, libs, or TypeScript version
changed.** `alias_name` is excluded from type equality and no `alias_id` is added,
so assignability is unchanged. Normal gate stays **78 PASS / 0 FAIL**;
`--strictSpans` stays **78 PASS / 0 FAIL**.

| Run | Before | After |
| --- | --- | --- |
| Normal gate | 78 PASS / 0 FAIL | **78 PASS / 0 FAIL** |
| `--strictMessages` | 76 PASS / 2 FAIL | **78 PASS / 0 FAIL** |
| `--strictSpans` | 78 PASS / 0 FAIL | **78 PASS / 0 FAIL** |
| both | 76 PASS / 2 FAIL | **78 PASS / 0 FAIL** |

**Fixes (display only):**

- **`JSX.IntrinsicElements` name in the unknown-tag TS2339**
  (`jsx-intrinsic-elements-basic`, `<unknown-tag />`). The message built the type
  name from `intrinsic_type.name()`, which expanded the resolved
  `JSX.IntrinsicElements` object structurally
  (`{ div: {…}; button: {…}; }`). tsc prints the nominal `JSX.IntrinsicElements`.
  Since the lookup already holds that constant, the diagnostic now passes it
  directly. `checks/jsx.rs` (`resolve_intrinsic_props_type`).
- **Nominal name on a peeled library-scoped interface for TS2741**
  (`module-augmentation-package-interface-basic`). `Client` is declared in a
  dependency (`node_modules/pkg`, a `DependencyDeclaration` → library-scoped) and
  augmented in-project to add `token`. A library-scoped non-generic interface is
  routed to a deferred `Type::Reference` whose body is expanded on peel by
  `LazyInstantiation::resolve`, which returned the structural object **without**
  attaching the declaration name — so the TS2741 "required in type 'X'" target
  (built from the *peeled* object's `.name()`) printed
  `{ id: string; token: string; }` instead of `Client`. The eager named-type path
  (`attach_object_alias_name`) tags the object, but the lazy path bypassed it.
  Now the lazy peel attaches the reference's display name as `alias_name` on a
  clean, non-empty resolved object before interning — mirroring the eager path and
  the §9 had_error/empty gate, and covering the generic case too (the stored
  display is the full `Box<string>` form, not a bare name). `infer/types/cache.rs`
  (`LazyInstantiation` gains a `display` field; `make_lazy_type_reference` populates
  it from the existing `display` argument).

**Commands run:**

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200                                   # 78/78
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages                  # 78/78
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictSpans                     # 78/78
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages --strictSpans    # 78/78
cargo fmt (jsx.rs, cache.rs) && cargo nextest run --workspace                         # 1391 passed
pnpm run oracle:test                                                                  # 21 passed
```

**Note (pre-existing, fixed in §11):** `real:ky` no longer reports 0/0 — it emits
a TS2345 / TS2554 pair. Those rows are argument-assignability (TS2345) and
argument-count (TS2554) diagnostics, which a display-only `alias_name` (excluded
from equality) cannot produce, so they are pre-existing. They are an unrelated
regression from intervening commits against the lib optional-argument surface;
§11 root-causes and fixes them. A separate `Uint8Array<any>` TS2322 regression
was confirmed to reproduce on `main` *without* this pass's changes (stashed the
two files, rebuilt, re-ran — identical rows).

## 11. Real-project regression fixes (`Uint8Array<any>` TS2322, ky TS2345/TS2554)

Follow-up to the §10 note. Three checker fixes restore `real:ky` to
**0 TypeScript / 0 surge-ts** diagnostics. Normal gate stays
**78 PASS / 0 FAIL** and both strict gates stay clean (78/78).

- **Nominal type-argument comparison for same-declaration references**
  (`surge-ts-types/src/assignability.rs`). Two instantiations of the *same*
  generic declaration now compare by their type arguments instead of their
  (often deeply self-referential) structural expansion, matching tsc. An `any`
  argument matches in either direction; an `unknown`/`GenuineUnknown` *source*
  argument is accepted because it is surge's sentinel for a generic the checker
  could not infer. This removes the `Uint8Array<any>` → `Uint8Array`
  TS2322 rows, where the two expansions spuriously diverged only in the
  `any`-argument positions. A companion arm tries a `from`-reference against
  each member of a union target *before* resolving the reference structurally
  (`Set<any>` vs `Set<string> | undefined`). Unit-tested with structurally
  opaque references so the tests prove the nominal path.
- **Concrete-instantiation gate fixed to check bound parameters, not scope
  depth** (`infer/types/resolve.rs`). `resolve_named_type` treated *any*
  non-empty `type_parameter_scopes` stack as non-concrete, but a plain
  (non-generic) function body also pushes an empty scope — so every library
  generic instantiated inside a function body (`new Uint8Array(...)`) was
  eagerly expanded into a degraded structural object (self-referential members
  collapse to `unknown`) instead of staying a nominal lazy reference. Now only
  a scope that actually binds a parameter marks the instantiation
  context-dependent.
- **Optional parameters accept `undefined` at call sites**
  (`checks/call/mod.rs`). An argument passed to an optional parameter
  (declared past the required count) is checked against `T | undefined`
  rather than the bare `T`, matching tsc. This removes ky's TS2345/TS2554
  pair on forwarding an optional argument.

Verified: `cargo nextest run --workspace` (1391 passed, plus new
`surge-ts-types` assignability unit tests), oracle sweep normal + both strict
flags (78/78 each), `pnpm run oracle:test` (21 passed), `real:ky` 0/0, `real:ofetch` 0 surge-only (the single only-TypeScript row is
tsc's TS5107 `esModuleInterop` deprecation notice, out of checker scope).
`real:zod` is unchanged from `main` (494 surge-only, capped; verified by
stash/rebuild/re-run) — a pre-existing gap, not affected by this pass.
