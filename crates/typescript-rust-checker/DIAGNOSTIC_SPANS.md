# Diagnostic Spans

This checker aims for predictable, useful diagnostic spans rather than byte-for-byte TypeScript LSP parity.

For the v0.68 emitted-diagnostic expansion phase, every newly emitted source-facing diagnostic should have at least one span test and should be checked against an oracle-backed fixture when feasible. If a diagnostic can cascade, pin that policy explicitly in the nearby tests.

The reference point for this phase is the TypeScript LSP underline behavior on the supported single-file and program checks. Span selection should prefer the smallest relevant token or expression, then fall back to a stable wrapper span when the smaller span is unavailable.

## Policy

| Diagnostic | Policy |
| --- | --- |
| TS2304 unresolved value identifier | identifier token span |
| TS2304 unresolved type name | type name token span |
| TS2305 missing default export | default import name span |
| TS2305 missing exported member | imported specifier name span |
| TS2307 unresolved ordinary relative module | module specifier string span |
| TS2307 non-relative ordinary module import | module specifier string span, or unsupported-module span when syntax is not modeled |
| TS2882 unresolved side-effect import | module specifier string span |
| TS2314 generic type arity mismatch | type reference name span, pinned |
| TS2315 non-generic type with type arguments | type reference name span, pinned |
| TS2344 invalid utility key constraint | type reference name span, pinned |
| typescript-rust::duplicate-type-parameter | duplicate type-parameter name span |
| TS7006 parameter implicitly any | parameter name span |
| TS7031 binding element implicitly any | object binding element local name span |
| TS7005 variable implicitly any | variable name span |
| TS2451 duplicate block-scoped declaration | each conflicting declaration name span (original and duplicate) |
| TS2393 duplicate function implementation | each conflicting function name span (original and duplicate) |
| TS2300 duplicate type declaration | duplicate declaration name span |
| TS2448 block-scoped variable used before declaration | offending read span |
| TS2454 variable used before assignment | offending read span |
| TS2588 assign to const | assignment target name span |
| TS2322 variable initializer mismatch | initializer expression span |
| TS2322 assignment mismatch | assignment value expression span |
| TS2322 return mismatch | return expression span |
| TS2322 object property value mismatch | property value expression span |
| TS2322 array element mismatch | array element expression span |
| TS2322 tuple element mismatch | tuple element expression span |
| TS2322 tuple length mismatch | array literal span or extra element span, pinned |
| TS2345 call argument mismatch | argument expression span |
| TS2554 wrong call arity | callee or call expression span |
| TS2349 non-callable | callee expression span |
| TS2339 missing property access | property name span |
| TS2339 invalid index receiver/out-of-range tuple index | offending index or receiver span, pinned |
| TS2536 invalid generic indexed-access key | key type / index type span when available, otherwise indexed-access type span |
| TS2538 invalid index type | index expression span |
| TS2353 excess object property | excess property name span |
| TS2741 missing required property | object literal span |
| TS2355/TS2366 missing return | function/method name span when available, otherwise no span |
| TS2362/TS2363 arithmetic operand mismatch | offending operand span |
| TS2365 invalid operator | operator or whole expression span, pinned |
| TS2367 no-overlap equality | operator or whole expression span, pinned |
| TS2872/TS2873 truthiness | condition or literal span |
| typescript-rust::duplicate-default-export | default keyword or export statement span, pinned |
| typescript-rust::unsupported-declaration | keyword or full statement span, pinned |
| typescript-rust::unsupported-module-syntax | import/export statement span, pinned |
| parser-error | parser-provided best-effort span when available; otherwise no span |

## Notes

- Successful relative imports bind the available type and/or value namespace. Namespace misuse underlines the local usage site, while failed module/export lookups underline the import/export token itself.
- TS2305 should underline the imported specifier name. TS2307 and TS2882 should underline the module specifier string.
- TS2314/TS2315 should stay pinned to the type reference name; v0.59 uses
  these as stable arity diagnostics for generic alias/interface references.
- TS2344 for invalid `Pick` keys is currently pinned to the `Pick` reference
  span. The parser does not retain a richer type-argument span for this path,
  so the checker keeps the underline stable at the utility reference and
  deduplicates repeated validation of the same alias body.
- Missing default exports underline the default-import identifier, and missing
  relative default-import modules underline the module specifier string.
- Missing re-export modules underline the module specifier string. Missing
  re-export members underline the exported specifier name.
- Namespace import property failures underline the property name, while
  namespace import module failures underline the module specifier string.
- Ambient namespace import missing-property failures underline the property access span on the consumer side.
- Star re-export module failures underline the module specifier string.
- Ambient modules support default imports when the default export exists; missing defaults use TS2305 and the default-import identifier span.
- Ambient module re-exports use the same specifier/member span rules as relative modules when the source ambient module is declared.
- `export * as Foo from ...` stays parser-safe or pinned rather than adding a
  separate span policy in this phase.
- Duplicate generic type parameters use a custom checker diagnostic and should
  underline the repeated name span, not the declaration keyword.
- TS2451 and TS2393 are emitted once per conflicting top-level declaration, not
  only at the redeclaration. tsc flags every participating declaration, so two
  `let x`/`function f` produce two diagnostics underlining each name span. The
  first declaration's span is recorded as it is registered and back-emitted when
  the duplicate is detected.
- A block-scoped (`let`/`const`) read inside its temporal dead zone is also
  definitely unassigned, so tsc reports both TS2448 and TS2454 at the same read.
  Both underline the offending read span.
- `TS7031` underlines the local binding name in object-pattern parameters.
  Aliases like `{ id: userId }` should underline `userId`, not `id`.
- Call-expression type arguments are parsed for syntax stability, but v0.59
  ignores them in checker flow, so no dedicated diagnostic span is emitted yet.
- Unresolved non-relative package-style imports emit TS2307. Missing side-effect imports emit catalog-backed
  TS2882 and use the same module-specifier span. `--stubExternalModules`
  suppresses both non-relative missing-module forms while preserving stubs.
  Resolved packages that are missing an export emit TS2305 on the imported specifier.
  Unsupported syntax still uses the pinned unsupported-module diagnostic.
- Ambient `declare module "pkg"` blocks and resolved package `.d.ts` entrypoints resolve before package stubbing; missing
  exports from a declared ambient module or package entrypoint still underline the imported specifier
  and use TS2305.
- Duplicate ambient module/global behavior is pinned rather than merged, so spans stay attached to the first-wins declaration or duplicate site used by the checker.
- Side-effect imports never bind names, so downstream unresolved-identifier diagnostics are still usage-site diagnostics.
- Nested contextual errors should prefer the most specific span available inside arrays, tuples, objects, calls, and property accesses.
- When the smaller span is unavailable, use the policy's pinned wrapper span and keep the code stable.
- The span-focused regression tests in `tests/spans.rs` and `tests/example_spans.rs` are the baseline for this policy.
- Use `cargo run -p typescript-rust-cli -- --showSpans <file>` to inspect spans during development.
- The TypeScript oracle comparison added in v0.60 compares code, file, and
  line/column first; it is a measurement baseline, not an exact span-parity
  contract. The pinned TypeScript version can shift the oracle output, so
  compare changes intentionally rather than treating the comparator as a
  semantic contract.

- **Unsupported Declaration Syntax**: `typescript-rust::unsupported-declaration` points to the syntax token in loaded declaration files.
- **Ambient Module Fallback**: Missing exports point to the import specifier.
- These custom `typescript-rust::*` diagnostics are catalog entries like the `TSxxxx` codes, but their spans are still decided at the callsite.
