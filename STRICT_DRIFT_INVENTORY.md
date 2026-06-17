# Strict Drift Inventory

Inventory of every remaining non-gating message-text and span/column drift in the
oracle preset sweep, classified for follow-up. **Documentation only — no checker
semantics, fixtures, gates, libs, or TypeScript version were changed.**

- Date: 2026-06-17
- TypeScript oracle version: pinned (unchanged)
- Scope: all 75 registered oracle presets, `--maxDiagnostics 200`

## 1. Summary

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
cargo fmt --check && cargo test --workspace && pnpm run oracle:test && pnpm run real:auth-kit   # all green, auth-kit 0/0
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
cargo fmt --check && cargo test --workspace && pnpm run oracle:test && pnpm run real:auth-kit   # all green, auth-kit 0/0
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
cargo fmt --check && pnpm run oracle:test && pnpm run real:auth-kit                   # green, auth-kit 0/0
# (cargo test --workspace skipped this pass at the user's request — runtime; the
#  changes are display-only and validated through the oracle sweeps.)
```
