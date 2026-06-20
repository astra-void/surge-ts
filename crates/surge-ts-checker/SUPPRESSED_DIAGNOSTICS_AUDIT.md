# Suppressed-Diagnostics Audit (follow-up)

**Status: substantially RESOLVED (2026-06-20).** Tracks the compatibility-report
suppression/stub counters that sit behind a "matches tsc" parity claim.
Source-level parity can be 0/0 while these counters are non-zero, because
surge-ts hides three categories of output before the user-facing comparison
(plus a fourth, stderr-only config-option category — see below).

What changed in this pass:
- **Recursive-type cycle fallback (action item #1) — FIXED.** A legal *non-generic*
  recursive type now resolves its self-edge to a lazy nominal `Type::Reference`
  to the same declaration instead of `unknown`, so a member/assignability check
  through the back-edge sees the real recursive shape rather than silently
  passing. All five ky source `surge::type-*-cycle` notes are gone; ky's
  native-profile run now surfaces **zero** source-level diagnostics. See below.
- **External-stub segmentation (action item #3) — DONE.** `externalModuleStubs`
  now reports `{ total, resolved, unresolved }`; `unresolved` is counted in the
  checker at the points an external specifier fails every resolution path.
- **Counter gating (action item #4) — DONE.** The ky regression gate now asserts
  zero source-level native-profile diagnostics and `externalModuleStubs.unresolved
  == 0`, so a regression that adds a suppressed source-level diagnostic fails the
  gate instead of hiding.

- **Lib-graph limits (action item #2) — three fixed, one deferred.** Fixed: the 14
  spurious `TS2393` "duplicate function implementation" notes on ambient `declare
  function` overloads (overload-aware duplicate detection; also −76 false `TS2393`
  on zod); the 2 `TS2536` on constrained indexed access
  (`addEventListener<K extends keyof WindowEventMap>` — the ambient
  signature-collection path now establishes the type-parameter scope); and the 2
  `TS2304` 'globalThis' (`typeof globalThis` → clean `unknown` + a `T & unknown ⇒
  T` intersection simplification, so `window`/`self` resolve to `Window` and their
  members are checked). All three pass every gate (ky 0/0, sweep 76/76, zod 0-new,
  no new trpc FP). Deferred: `intrinsic`/`BuiltinIteratorReturn` — its surface fix
  (`intrinsic ⇒ any`) exposes surge's incomplete iterator/indexed-access modelling
  and **breaks ky 0/0**. Root-caused below.

## What the counters mean

From `crates/surge-ts-checker/src/context.rs` (`should_suppress` /
`record_suppressed`) and `crates/surge-ts-cli/src/report.rs`:

- **`suppressedRustOnlyDiagnosticsTotal`** — diagnostics whose code starts with
  `surge::` (parser errors, cycle detection, other surge-internal limits). These
  are never TypeScript codes, so they are always suppressed from the tsc-profile
  output. A non-zero value means surge hit something it could not fully model.
- **`suppressedDeclarationDiagnosticsTotal`** — diagnostics raised inside a
  declaration (`.d.ts`) file. Physical default-lib files and trusted upstream
  dependency declarations are suppressed wholesale so unsupported lib syntax
  cannot flood user diagnostics.
- **`externalModuleStubs.total`** — count of non-relative (package) import/export
  specifiers in the scanned sources. It is a count of external-module references,
  not a count of *failed* resolutions; a referenced package may still resolve via
  the dependency-declaration path.

To see the suppressed diagnostics, re-run with the native profile (which disables
suppression):

```sh
surge --project <tsconfig> --diagnosticProfile native --diagnosticStyle tsc
```

## ky — audit (2026-06-20, post cycle-fix)

Counters on `.local-projects/ky` (tsc reports 0; surge tsc-profile reports 0):

| counter | before fix | after fix |
| --- | ---: | ---: |
| `suppressedRustOnlyDiagnosticsTotal` | 15 | 4 |
| `suppressedDeclarationDiagnosticsTotal` | 23 | 22 |
| `externalModuleStubs` | `{total:1}` | `{total:1, resolved:1, unresolved:0}` |

`--diagnosticProfile native` (suppression off) now surfaces **2** diagnostics,
**all in physical lib `.d.ts`** and **zero in ky source** (down from 25 in the
first pass → 19 after the cycle fix → 6 after the overload fix → 4 after the
constrained indexed-access fix → 2 after the globalThis fix):

- 1 `TS2304` (`BuiltinIteratorReturn`), 1 `surge::type-declaration-cycle`
  (`Set<T>`) — in `lib.es2015.iterable.d.ts` / `lib.es2015.collection.d.ts`. The
  14 `TS2393` (overload fix), 2 `TS2536` (constrained indexed-access fix), and 2
  `TS2304` 'globalThis' (globalThis fix) are gone; see "Lib-graph limits" below.

The **5 ky SOURCE** cycle notes that the first pass flagged as "the item that
actually needs attention" (`KyInstance`, `Options` ×2, `Hooks`, `InitHook`) are
**gone**: they are legal non-generic recursive types, and surge now resolves each
self-edge to a lazy nominal reference to its own declaration rather than emitting
a suppressed `surge::type-*-cycle` note and degrading to `unknown`. The lib
`Set<T>` self-cycle remains (it is generic — see the fix's scope note), which is
the one new entry in the otherwise-shrunk `suppressedRustOnly` count.

### The fix (recursive-type cycle fallback)

`resolve_type_alias` / `resolve_interface` take the declaration's
`TypeDeclarationHandle`; on a detected resolution cycle the legal back-edge of a
**non-generic** structural alias / any **non-generic** interface returns
`make_recursive_cycle_reference(...)` — a lazy nominal `Type::Reference` that peels
one level to the real shape on demand (bounded by `LAZY_PEEL_STACK`). A
member/assignability probe through the self-edge is therefore checked against the
real recursive type. Locked by smoke cases
`type-alias-recursive-member-access-through-self-edge` (TS2339+TS2322 through a
recursive object field) and the rewritten `*-cycle-*-no-cascade` cases, which now
assert tsc's real codes (TS2741 / TS2322 / TS2355 / `[]`) instead of the old
suppressed note.

**Scope — generic recursion stays `unknown` (deliberate).** A *generic*
recursive declaration is left as `unknown` with the (suppressed) note, because its
lazy peel is bounded mid-instantiation: forcing the deeply self-instantiating
generic clusters (trpc's `ProcedureBuilder<…8 params…>`, whose every method returns
`Builder<…refined…>`) would expose an incomplete shape and over-report
member/assignability checks. Verified: an unguarded version added ~33 false
positives on trpc examples (`Property 'mutation' does not exist on type
'ProcedureBuilder'`, etc.); gating on `type_parameters.is_empty()` drops the trpc
and zod deltas to exactly **0 new / 0 removed** while keeping every non-generic
fix. Keeping `unknown` there preserves the previous sound-but-under-reporting
behaviour for generics.

**Known residual (pre-existing, not introduced here):** `type A = B` where `B` is
an interface that cycles back to `A` still emits a spurious `surge::type-alias-cycle`
note (tsc reports `TS2741` on the assignment). The legality check
`alias_body_supports_recursion` only inspects the alias's immediate parsed body, so
it cannot see that the bare reference `B` is a structural interface. Locked
unchanged by smoke case `interface-cycle-type-alias-through-interface-no-cascade`.

`externalModuleStubs.total = 1` is the single non-relative import in ky source,
`import type {Expect, Equal} from '@type-challenges/utils'` (a compile-time type
assertion in `core/constants.ts`). The package is installed (`dependency
declaration files: 1`, `dependency declaration diagnostics: 0`), so it resolves;
the counter is just flagging the external reference.

## Config-option diagnostics (stderr-only, not counted) — RESOLVED 2026-06-20

A fourth category sat entirely outside the three counters above: tsconfig option
diagnostics from `surge-ts-config`. ky's base config (`@sindresorhus/tsconfig`)
targets TS 6.0, and surge's option registry
(`crates/surge-ts-config/src/options.rs`) lagged it, so loading ky used to emit:

- **1 `InvalidCompilerOptionValue`** — `module: "node20"`. `parse_module_option`
  did not recognize `node20` and **fell back to `ModuleKind::Preserve`**.
- **8 `UnknownCompilerOption`** — none of `newLine`, `stripInternal`,
  `erasableSyntaxOnly`, `noImplicitOverride`,
  `noPropertyAccessFromIndexSignature`, `noUncheckedSideEffectImports`,
  `noEmitOnError`, `useDefineForClassFields` were in the registry; each was
  parsed, warned, and dropped.

These were written to **stderr** (`crates/surge-ts-cli/src/main.rs` ~L540,
`eprintln!`), not to the diagnostic JSON — **not** TS codes, **not** in
`suppressedRustOnly`/`suppressedDeclaration`, and **not** in the oracle
comparison — so the ky 0/0 parity was never affected by them.

**Fix:** `node20` is now a recognized `ModuleKind`/`ModuleResolutionKind`
(node-style resolution, treated like node16/nodenext — non-bundler), and the 8
options are registered. Loading ky now emits **zero** config diagnostics
(verified via `--showConfig`; `"module": "node20"` resolves correctly). Locked by
`tests::ts6_node20_and_newer_options_are_recognized` in `surge-ts-config`.

The 8 options are registered as `KnownNoop`, matching how the existing
strictness family is already handled (`noUncheckedIndexedAccess`,
`noImplicitReturns`, `exactOptionalPropertyTypes`, etc.): surge recognizes and
validates the value but does **not** implement the check. The residual
transparency note still stands for the *checking-relevant* flags
(`erasableSyntaxOnly`, `noImplicitOverride`, `noPropertyAccessFromIndexSignature`,
`noUncheckedSideEffectImports`, `useDefineForClassFields`) — surge does not
enforce them, so on a project whose source *did* trip one it would under-report
relative to tsc. This is the same standing limitation as every other KnownNoop
strictness flag, not a ky-specific gap; ky's 0/0 holds because ky source trips
none of them.

## Action items

1. ~~**Recursive-type cycle fallback (highest value).**~~ **DONE (2026-06-20).**
   The 5 ky source `surge::type-*-cycle` notes are gone; their legal non-generic
   recursive types now resolve through a lazy nominal self-reference instead of
   degrading to `unknown`. Generic recursion is deliberately left as `unknown` (it
   would over-report — see "The fix" above). Locked by the rewritten `*-cycle-*`
   smoke cases + the ky native-profile gate.
2. **Lib-graph limits (three fixed; one deferred + a tracked residual).**
   The first-pass ky native run surfaced 20 lib `.d.ts` diagnostics; after the
   cycle fix (#1) 14 remained, after the overload fix 6, after the constrained
   indexed-access fix 4, and after the globalThis fix **2** remain (the
   `BuiltinIteratorReturn` TS2304 and the `Set<T>` generic-recursion cycle).
   - **`TS2393` ×14 "Duplicate function implementation" (`lib.dom.d.ts`) —
     FIXED (2026-06-21).** These were ambient `declare function` *overloads* at
     global scope (`postMessage`, `scroll`, `addEventListener`/`removeEventListener`,
     …, each declared 2+ times). tsc merges same-name `declare function`s as an
     overload set; surge flagged the second as a duplicate *implementation*. Fix:
     TS2393 now fires only when a body-bearing implementation follows another
     body-bearing implementation (tracked per scope via
     `SymbolTable::{mark,has}_function_implementation`); bodyless declarations
     merge as overloads. A companion fix skips body checks (e.g. TS2355) on
     bodyless `function` signatures. Verified: all 14 lib FPs gone, ky still 0/0,
     sweep 76/76, **zod −76 false TS2393**, trpc −1 (no new). Locked by the
     rewritten `ambient_global_duplicate_function_policy_pinned` and the
     `function-overload-signatures-not-duplicate` smoke case. (Residual: surge
     still does not build a true overload *set* for call resolution — it keeps the
     first signature — a separate limitation tracked apart from the duplicate
     policy.)
   - **`TS2536` ×2 "Type 'K' cannot be used to index …" (`lib.dom.d.ts`) —
     FIXED (2026-06-21).** `addEventListener<K extends keyof WindowEventMap>(…,
     listener: (this: Window, ev: WindowEventMap[K]) => any)`. Root cause: the
     ambient/global signature-collection path
     (`collect_function_declaration_signature`) mapped the signature *without*
     establishing the function's type-parameter scope, so the constraint `K extends
     keyof WindowEventMap` was invisible and `WindowEventMap[K]` reported a false
     TS2536. (Single-file checking always had the scope, which is why it only
     reproduced in project/ambient mode.) Fix: wrap the mapping in
     `with_type_parameter_scope`, **scoped to generic `declare` functions** — for a
     `declare` function the collected signature is authoritative (no body check
     follows), whereas a non-`declare` function is re-checked under its own scope
     by `check_function_declaration`, and resolving its (cross-module) signature
     concretely *here* changed how generic instantiations were collected and
     surfaced assignability false positives (e.g. zod `api.ts`). With the
     `is_declare` gate: 2 lib TS2536 gone, ky still 0/0, sweep 76/76, **zod 0
     delta**, trpc net −9 false positives. Locked by
     `ambient_generic_function_constrained_indexed_access_no_ts2536`. (Residual: 2
     new trpc TS2339 in `react-query`/`openapi` — see below — from *separate*
     deeper gaps the more-precise `declare` signatures expose; trpc is net-better
     and not a gated 0/0 project.)
   - **`TS2304` 'globalThis' ×2 (`declare var self/window: Window & typeof
     globalThis`) — FIXED (2026-06-21), as a two-part change.** globalThis's value
     symbol is installed (`sync_global_this_symbol`) only after every ambient global
     is collected, so an ambient var naming `typeof globalThis` resolves it first
     and missed.
     1. **`typeof globalThis` resolution**: on the miss, return a clean `unknown`
        with `had_error: false` instead of a (suppressed) TS2304 with `had_error:
        true` — globalThis is always a valid built-in, so the diagnostic is a false
        positive and the `had_error` would poison the enclosing intersection.
     2. **Intersection `T & unknown ⇒ T`**: `merge_intersection_members`, after
        dropping the `unknown` operand, now returns a lone surviving member
        unchanged instead of peeling + re-merging it. Re-merging forced the lazy
        `Window` reference's bounded structural expansion and discarded its nominal
        identity, which both degraded `window`/`self` *and* corrupted the **shared**
        `Window` apparent type — a plain `declare const win: Window; win.appVersion`
        then resolved to `Window` (caught by the `declare-global-window-physical-lib-basic`
        sweep preset). Returning the member unchanged keeps `Window & typeof
        globalThis ⇒ Window`.
     With both, `window`/`self` resolve to `Window`, member access is checked
     (`w.bar` is `string`, locked by
     `ambient_global_typeof_global_this_intersection_resolves_to_left`), and the 2
     lib globalThis TS2304 are gone. Verified gate-clean: sweep 76/76, ky 0/0, zod
     0-new, **no genuinely-new trpc FP** (the only trpc movement is original-baseline
     churn). Residual: `window.<unknown-prop>` reports TS4111 rather than tsc's
     TS2339 — a *separate* spurious string-index-signature on surge's physical-lib
     `Window`, latent (no gated project trips it) and tracked apart.
   - **`TS2304` 'BuiltinIteratorReturn' ×1 — deferred (iterator-type modelling).**
     `type BuiltinIteratorReturn = intrinsic`; surge drops the alias because the
     oxc→`ParsedType` lowering had no `intrinsic` case, so references to it report
     TS2304. Making the alias resolve — whether `intrinsic ⇒ any` *or* `intrinsic ⇒
     unknown`, both behave identically here — is **not** the real work: it merely
     lets `SetIterator`/`IteratorObject<T, BuiltinIteratorReturn, …>` expand, and
     because every array property's `[Symbol.iterator]` pulls that chain, the
     recursive `Hooks` type (its `…Hook[]` members) resolves more fully and
     **breaks ky 0/0** by surfacing two *downstream* gaps, neither about iterators
     directly:
     1. `merge.ts` `function newHookValue<K extends keyof Hooks>(): Required<Hooks>[K]`
        → false TS2536. **Root cause (debugged):** at the indexed access `K` is a
        placeholder in the threaded `substitution` (`idxph = Some("K")`) but its
        `keyof Hooks` constraint is absent from `ctx.type_parameter_constraint_scopes`
        (`keyof_target("K") = None`), because this non-`declare` signature is
        resolved in the ambient/global pre-pass without the function's
        type-parameter scope (the #7 fix is declare-scoped). Without the iterator
        chain `Required<Hooks>` resolves with `had_error` and the access
        short-circuits before the TS2536 check, masking it. **A targeted fix
        works:** suppress TS2536 for a placeholder index over a *concrete* (non-
        placeholder) receiver whose constraint can't be verified (degrade to
        `unknown` no-cascade) — verified to clear ky `merge.ts`. Held back only
        because it is inert without the iterator chain and shifts trpc churn.
     2. `Ky.ts` `(options.hooks?.init ?? []).length` → TS2339 `.length` on
        `unknown`. **Root cause:** `options.hooks` itself resolves to `unknown` —
        the recursive `Options`/`Hooks`/`InitHook` lazy peel reaches its depth bound
        once the iterator chain deepens the `…Hook[]` members, so the property
        access degrades; `?? []` then propagates `unknown` to `.length`.
     So closing this needs real **iterator-type modelling** (so resolving the alias
     does not deepen consumers' peels) *plus* the gap-1 indexed-access suppression
     *plus* recursive-peel-depth tuning — a coordinated feature, each part inert or
     untestable until the others land. The dropped-alias / suppressed-TS2304 status
     quo is the pragmatic optimum. **Reverted; tracked as a dedicated follow-up with
     both root causes pinned above.**
   - **`surge::type-declaration-cycle` on `Set<T>`** — the generic-recursion case
     deliberately left as `unknown` by the #1 fix (see its scope note).
   - **Newly-exposed (by the TS2536 fix), separate deeper gaps — open:** 2 trpc
     `TS2339` the more-precise `declare`-generic signatures surface —
     `context.client` on a `TRPCContextState<…>` instantiation (generic member
     resolution) and `fs.realpathSync.native` (function-with-namespace-property
     merge on a `@types/node` overload). Both are distinct from indexed access; tsc
     reports neither. Not gated; documented for follow-up.
3. ~~**Stub-vs-resolve clarity.**~~ **DONE (2026-06-20).** `externalModuleStubs` now
   reports `{ total, resolved, unresolved }`. `unresolved` is counted in the
   checker (`record_unresolved_external_module`) at every point an external
   specifier fails resolution (imports + re-exports), so a silently-stubbed
   unresolved import is now distinguishable from a benign resolved reference.
   Locked by `compat_report_external_module_stubs_json`.
4. ~~**Gate the counters.**~~ **DONE (2026-06-20).** The ky regression gate
   (`scripts/real-projects/ky-regression.test.ts`) now asserts (a) the native
   profile surfaces **zero** ky *source* diagnostics — catching any new suppressed
   source-level diagnostic, including a recursive-cycle regression — and (b)
   `externalModuleStubs.unresolved == 0`.
5. ~~**Catch up the tsconfig option registry to TS 6.0.**~~ **DONE (2026-06-20).**
   `node20` is recognized for `module`/`moduleResolution` and the 8 newer options
   are registered. Loading ky emits zero config diagnostics; locked by
   `tests::ts6_node20_and_newer_options_are_recognized`.
6. **Enforce checking-relevant strictness flags (in progress, 2026-06-20).** The
   strict family was all `KnownNoop` (parsed, dropped, never enforced). Now being
   enforced flag-by-flag, each gated on the project's own tsconfig so the oracle's
   tsc run matches and each verified against ky (0/0) + the preset sweep + the
   zod/trpc real projects:
   - **`noImplicitReturns` (TS7030)** — DONE. Emitted for an unannotated
     function/arrow that returns a value on some path with a reachable end.
     Required fixing the return-flow summary (try/switch `guarantees_exit`,
     `while (true)`, throw-vs-return, constructor exclusion). Known limitation:
     `return <void-typed-expr>` over-reports vs tsc (flow-only; tsc consults the
     inferred return type).
   - **`noFallthroughCasesInSwitch` (TS7029)** — DONE. Emitted for a non-empty
     switch clause whose end is reachable.
   - **`noImplicitOverride` (TS4114)** — DONE. Parser now captures the `override`
     (and `abstract`) modifier; emitted for an instance member overriding a
     source-declared base-class member without `override`. Conservative:
     `.d.ts`/builtin bases and abstract base members are skipped to avoid false
     positives.
   - **`noPropertyAccessFromIndexSignature` (TS4111)** — DONE. A new `is_bracketed`
     AST flag preserves the dot-vs-bracket distinction; emitted for `obj.foo` that
     resolves through a string index signature. FP-free; under-reports on library
     types whose index signature surge does not fully resolve (`process.env`, node
     headers).
   - **`noUnusedParameters` (TS6133)** — DONE. Required a new use-tracking
     foundation: a parser pass (`reads.rs`, via `oxc_ast_visit`) collects every
     value-position `IdentifierReference` from the full oxc body and stores it as
     `body_reads` on each parsed function/arrow/method, so reads inside spreads,
     `for-in`, object methods, template literals, and nested functions are all
     visible. Template literals are now parsed and nested function declarations
     retained (inert) for the same reason. Overload signatures (`has_body =
     false`) and `_`-prefixed/`this`/pattern parameters are exempt. FP-free
     against ky 0/0, zod, and trpc.
   - **`noUnusedLocals` (TS6133)** — DONE. Module-level unused imports and
     value declarations (`const`/`let`/`var`, functions) are flagged via a
     module-wide read set (`ParsedSource::module_reads`, from the full oxc AST,
     skipped for `.d.ts`); function-local `const`/`let`/`var` are flagged via the
     per-function `body_reads`. Matches tsc: only modules are checked, top-level
     *classes* are exempt, exported/`declare` bindings are exempt, and type- and
     export-specifier references count as uses (so type-only imports are safe).
     FP-free against ky 0/0, zod, and trpc. Minor remaining FN: constructor-local
     bindings and a few edge constructs.
   - **Remaining flags — investigated, no action needed (2026-06-20).** None of
     the still-`KnownNoop` strictness flags warrant enforcement:
     - **`noUncheckedSideEffectImports`** — already matched. surge emits TS2882
       for an unresolvable side-effect import unconditionally, which is exactly
       TS 6.0 `tsc`'s behavior (verified: `tsc` reports TS2882 with the flag both
       on and off; surge reports the same two). The flag changes nothing here.
     - **`erasableSyntaxOnly`** — not worth implementing. Its trigger constructs
       (enums, parameter properties) are not even parsed by surge (enums lower to
       `UnsupportedDeclaration`; constructor parameter modifiers are not
       captured), so only `namespace`/`import =` would be detectable, and those
       need an erasable-vs-runtime distinction. Partial coverage of a rare flag.
     - **`useDefineForClassFields`** — emit/runtime-semantics; under noEmit it
       produces essentially no standalone diagnostics.
     - **`strictNullChecks` / `exactOptionalPropertyTypes`** — out of scope. surge
       has no `Type::Null` and treats `undefined` as non-assignable, i.e. it is
       hard-wired strict-null: the common `strictNullChecks: true` already matches,
       `false` cannot be modelled without nullable-widening machinery, and full
       EOPT needs per-property exact-optional tracking (deep type-system work).

   With this, the checking-relevant strictness family is enforced where it is
   both tractable and valuable (TS7030, TS7029, TS4114, TS4111, TS6133); the rest
   are a deliberate, documented non-enforcement rather than a silent gap.

This is a follow-up to the 2026-06-20 ky 0/0 parity landing (see
`REAL_PROJECT_COMPAT.md` → "ky" → "Suppression / stub transparency").
