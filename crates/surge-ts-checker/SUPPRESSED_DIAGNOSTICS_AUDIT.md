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

## Config-option diagnostics (stderr-only, not counted)

A fourth category sits entirely outside the three counters above: tsconfig
option diagnostics from `surge-ts-config`. ky's base config
(`@sindresorhus/tsconfig`) targets TS 6.0, and surge's option registry
(`crates/surge-ts-config/src/options.rs`) lags it, so loading ky emits:

- **1 `InvalidCompilerOptionValue`** — `module: "node20"`. surge's
  `parse_module_option` does not recognize `node20` and **falls back to
  `ModuleKind::Preserve`** (`moduleResolution: "node16"` *is* recognized).
- **8 `UnknownCompilerOption`** — none of `newLine`, `stripInternal`,
  `erasableSyntaxOnly`, `noImplicitOverride`,
  `noPropertyAccessFromIndexSignature`, `noUncheckedSideEffectImports`,
  `noEmitOnError`, `useDefineForClassFields` are in the registry; each is parsed,
  warned, and dropped.

These are written to **stderr** (`crates/surge-ts-cli/src/main.rs` ~L540,
`eprintln!`), not to the diagnostic JSON. They are **not** TS codes, **not** in
`suppressedRustOnly`/`suppressedDeclaration`, and do **not** enter the oracle
comparison — so the ky 0/0 parity is unaffected by them. The transparency
concern is what the dropped flags *would* have checked:

- **Emit / noEmit-irrelevant (safe to ignore):** `newLine`, `stripInternal`,
  `noEmitOnError`, and `declaration` semantics. These never produce type
  diagnostics under a noEmit checker.
- **Checking-relevant (silently un-enforced):** `erasableSyntaxOnly` (TS5.8 —
  errors on non-erasable syntax), `noImplicitOverride` (TS4114),
  `noPropertyAccessFromIndexSignature` (TS4111), `noUncheckedSideEffectImports`,
  and `useDefineForClassFields` (class-field init semantics). surge does not
  honor these, so on a project whose source *did* trip one, surge would
  **under-report** relative to tsc. ky's 0/0 holds only because ky source
  happens not to trip any of them — surge is matching tsc's *result*, not
  enforcing tsc's *configured strictness*.

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
5. **Catch up the tsconfig option registry to TS 6.0.** Add `module: "node20"`
   to `parse_module_option` (currently defaults to `Preserve`) and register the
   8 unknown options. For the checking-relevant ones (`erasableSyntaxOnly`,
   `noImplicitOverride`, `noPropertyAccessFromIndexSignature`,
   `noUncheckedSideEffectImports`, `useDefineForClassFields`), decide per-flag
   whether to enforce or to mark `KnownNoop` — silently dropping a strictness
   flag means surge can under-report where tsc would error. Until then, the ky
   0/0 claim matches tsc's *result* but not its *configured strictness*.

This is a follow-up to the 2026-06-20 ky 0/0 parity landing (see
`REAL_PROJECT_COMPAT.md` → "ky" → "Suppression / stub transparency").
