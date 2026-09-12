# Real Project Compatibility — historical notes

**Historical record. None of this describes current behavior.** These sections
were moved out of [REAL_PROJECT_COMPAT.md](../../REAL_PROJECT_COMPAT.md) so
that the version-tagged milestone notes and the superseded baseline lists
cannot be mistaken for the current state.

For what is true now, read [CURRENT_STATUS.md](../../CURRENT_STATUS.md). For
the current measured real-project results and their history, read
[REAL_PROJECT_COMPAT.md](../../REAL_PROJECT_COMPAT.md).

The `v0.x` / `v1.x` labels below are **internal milestone labels**, not
releases, tags, or crate versions — see
[CURRENT_STATUS.md § Versioning](../../CURRENT_STATUS.md#versioning).

Known corrections to the text below, verified on 2026-09-01 at commit
`37dfb3a`:

- `baseUrl` non-relative specifier resolution **is** supported in the loader.
- `export =`, `import … = require(…)`, enums, and namespaces **are** supported
  and gated by oracle presets.
- Automatic `@types` discovery **is** implemented to TypeScript 6/7 semantics
  (including the `types: ["*"]` wildcard) — see
  [crates/surge-ts-cli/AUTO_TYPES.md](../../crates/surge-ts-cli/AUTO_TYPES.md).
- Transitive `/// <reference types="…" />` loading from dependency declaration
  files **is** implemented.
- Standard/DOM globals come from the physical `lib*.d.ts` graph loaded by
  default; the generated subset is only the fallback.

---

## v0.60 – v0.85 milestone log (historical)

The version-tagged notes below are historical, recording how the checker reached
this state. Their wall-clock medians and "synthetic built-ins" / "generated
default-lib" descriptions reflect the measurement and lib model in effect at the
time, not necessarily current behavior.

`v0.60.1` is still an instrumentation baseline for real-project compatibility,
not a claim that large TypeScript packages pass. `v0.60` adds a TypeScript
oracle comparison harness on top of that baseline so we can measure the current
checker against a pinned compiler without changing the checker to chase parity.

v0.82 is a project visibility and file-discovery hardening phase. It does not
claim full real-project parity. The goal is to make silent zero-file project
comparisons impossible, especially when `tsc` sees `.tsx`, `.mts`, `.cts`,
`.d.ts`, and nested `examples/**` inputs that the Rust loader might otherwise
miss. `.tsx` visibility is not the same as JSX or React type support. A later
parser-safe JSX slice adds JSX element/fragment/attribute parsing and a
conservative `JSX.Element` inference (walking `{...}` containers and component
tags for ordinary diagnostics) without `JSX` namespace resolution, intrinsic
prop validation, React globals, or the JSX transform.

## Per-feature notes (historical)

v0.85 adds a generated default-lib foundation. It does not load the full official TypeScript lib files at runtime; instead it generates a small supported subset from the local TypeScript package and loads those generated declarations as ambient default libs. `noLib: true` disables the generated default libs. Full lib.d.ts parity, Node discovery, and `@types` discovery remain future work.

v0.68.1 hardens the diagnostic coverage metadata, ensuring that `support = "emitted"` accurately reflects current checker capabilities and is backed by testing.

v0.77.1 implements non-null assertions and a parser-safe `as const` foundation under the default `tsc` diagnostic profile. Literal types and tuple constraints are preserved on primitive literals and object/array properties for `as const` expressions. `satisfies` with `as const` behaves correctly. Optional chaining AST evaluation now correctly propagates the `undefined` short-circuit across subsequent non-null assertions (e.g. `a?.b!.c` evaluates to `C | undefined`).
v0.74.1 supports nested optional property/call chains in a conservative way, and optional element access for arrays and tuples. Every optional chain segment still widens the result with `undefined`. `??` removes `undefined` only in the supported subset. `null`-accurate semantics and control-flow narrowing remain unsupported. `ignoreDeprecations` is not used in committed fixtures because TS 7-oriented compatibility should not hide deprecated option behavior.
v0.70 supports package declaration subpath entrypoints.
v0.69 supports narrow bare package declaration entrypoints.
v0.69.1 hardens/refactors this support. v0.72/v0.72.1 used synthetic built-ins, not physical `lib.d.ts` (since superseded by physical-lib loading). `Array<T>` and `ReadonlyArray<T>` are modeled enough to preserve element diagnostics. v0.81 adds narrow synthetic lowering for `Record`, `Partial`, `Pick`, and `Omit` on top of the mapped-type foundation introduced in v0.80.1. This is still not full utility-type support: `Required`, `Readonly`, `ReturnType`, `Parameters`, `Awaited`, and conditional-type-backed utilities remain unsupported or synthetic noise reducers. Full index signatures remain unsupported, while any narrow `Record<string, T>` / string-index fallback stays limited to oracle-backed narrow paths when the implementation explicitly supports it. Standard/DOM globals now come from the physical `lib*.d.ts` graph loaded by default (the generated subset, which v0.85 introduced, is the fallback); full `lib.d.ts` parity and automatic Node/`@types` discovery remain future work. `noLib: true` disables both the physical and generated default libs, keeping standard/DOM globals unavailable.
Supported (declaration resolution): types, typings, index.d.ts, bare scoped/unscoped packages, exact declaration subpaths, exact `exports["."].types` / `exports["./x"].types` declaration targets, physical `lib*.d.ts` loading by default, and standard/DOM globals sourced from those loaded libs.
Out of scope (declaration resolution): exports runtime conditions, main, wildcard exports, automatic `@types` discovery, baseUrl resolution, JS runtime entrypoints, rootDirs, and project references. (`typesVersions` resolution later became supported; see the current state section above.)
## Historical baseline snapshot (superseded)

**Superseded.** This list mixes already-landed features into its
"intentionally avoids" section — see the corrections at the top of this file
and [CURRENT_STATUS.md](../../CURRENT_STATUS.md#known-limitations) for the
current support surface.

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

