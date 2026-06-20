# Suppressed-Diagnostics Audit (follow-up)

**Status: OPEN.** Tracks the compatibility-report suppression/stub counters that
sit behind a "matches tsc" parity claim. Source-level parity can be 0/0 while
these counters are non-zero, because surge-ts hides three categories of output
before the user-facing comparison (plus a fourth, stderr-only config-option
category — see below). A product-grade parity claim must confirm none of them
masks a real source-level miss.

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

## ky — first-pass audit (2026-06-20)

Counters on `.local-projects/ky` (tsc reports 0; surge tsc-profile reports 0):

| counter | value |
| --- | ---: |
| `suppressedRustOnlyDiagnosticsTotal` | 15 |
| `suppressedDeclarationDiagnosticsTotal` | 23 |
| `externalModuleStubs.total` | 1 |

`--diagnosticProfile native` surfaces 25 otherwise-suppressed diagnostics:

- **20 in physical lib `.d.ts`** (`lib.dom.d.ts` ×18, `lib.es2015.iterable`,
  `lib.es2015.collection`). These are surge parser/checker limits on upstream lib
  syntax, suppressed because `PhysicalDefaultLib` is trusted. Benign for the
  *source* claim, but they mean surge mis-handles ~20 constructs in the lib graph,
  which can quietly degrade the resolved types user code sees.
- **5 in ky SOURCE** — all `surge::type-alias-cycle` /
  `surge::type-declaration-cycle`:
  - `source/types/ky.ts:5` `KyInstance`
  - `source/types/options.ts:399` `Options` and `source/types/hooks.ts:366` `Options`
  - `source/types/options.ts:91` / `source/types/hooks.ts:28` `Hooks` / `InitHook`

  These are **legal self-referential / mutually-recursive types** that tsc accepts
  without complaint. surge's cycle detector emits a (suppressed) `surge::` note
  and falls back to a degraded resolution for the cyclic type. This is the item
  that actually needs attention: the 0/0 source claim holds only because these are
  `surge::` codes, not TS codes — but the underlying types are not fully modelled
  (several ky fixes had to work around `KyInstance`/`Options` resolution).

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

1. **Recursive-type cycle fallback (highest value).** Investigate the 5
   `surge::type-*-cycle` notes on legal recursive ky types (`KyInstance`,
   `Options`, `Hooks`, `InitHook`). Confirm whether the cyclic fallback degrades
   the resolved type, and whether any downstream user-facing check silently
   under-reports because of it. tsc models these without a cycle error.
2. **Lib-graph limits.** Enumerate the ~20 suppressed lib `.d.ts` diagnostics by
   code; decide which are genuinely unsupported lib syntax (acceptable to suppress)
   versus surge bugs that could corrupt a resolved global/DOM type.
3. **Stub-vs-resolve clarity.** `externalModuleStubs` counts references, not
   failures. Either rename/segment it into "referenced" vs "stubbed (unresolved)"
   so a real unresolved import is distinguishable, or add a separate
   unresolved-external counter. For a parity claim, an *unresolved* external
   module is the risky case; a resolved one is not.
4. **Gate the counters.** Once audited, consider asserting expected suppression
   counts (or `== 0` for ky source-file `surge::` diagnostics) in the `real:ky`
   regression gate so a regression that adds a new suppressed source-level
   diagnostic is caught, not hidden.
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
   - **Remaining:** `noUnusedLocals`/`noUnusedParameters` (TS6133) needs a new
     use-tracking pass (no existing reference counting) with several FP-exemption
     rules (exports, ambient, `_`-prefixed params, destructuring) — a larger
     subsystem, not a flow-summary reuse. `noImplicitOverride` (TS4114) needs the
     parser to capture the `override` keyword + base-member resolution.
     `noPropertyAccessFromIndexSignature` (TS4111) needs index-signature
     provenance on property access. `erasableSyntaxOnly` /
     `noUncheckedSideEffectImports` are low-value; `useDefineForClassFields` is
     emit-semantics. `strictNullChecks` / `exactOptionalPropertyTypes` are out of
     scope (surge is hard-wired strict-null; see the verdict above).

This is a follow-up to the 2026-06-20 ky 0/0 parity landing (see
`REAL_PROJECT_COMPAT.md` → "ky" → "Suppression / stub transparency").
