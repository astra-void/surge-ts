# Diagnostic Spans

This checker aims for predictable, useful diagnostic spans rather than byte-for-byte TypeScript LSP parity.

The reference point for this phase is the TypeScript LSP underline behavior on the supported single-file and program checks. Span selection should prefer the smallest relevant token or expression, then fall back to a stable wrapper span when the smaller span is unavailable.

## Policy

| Diagnostic | Policy |
| --- | --- |
| TS2304 unresolved value identifier | identifier token span |
| TS2304 unresolved type name | type name token span |
| TS2305 missing default export | default import name span |
| TS2305 missing exported member | imported specifier name span |
| TS2307 unresolved relative module | module specifier string span |
| TS2307 non-relative module import | module specifier string span, or unsupported-module span when syntax is not modeled |
| TS2314 generic type arity mismatch | type reference name span, pinned |
| TS2315 non-generic type with type arguments | type reference name span, pinned |
| typescript-rust::duplicate-type-parameter | duplicate type-parameter name span |
| TS7006 parameter implicitly any | parameter name span |
| TS7005 variable implicitly any | variable name span |
| TS2451 duplicate block-scoped declaration | duplicate declaration name span |
| TS2393 duplicate function implementation | duplicate function name span |
| TS2300 duplicate type declaration | duplicate declaration name span |
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
| TS2353 excess object property | excess property name span |
| TS2741 missing required property | object literal span |
| TS2355/TS2366 missing return | function span if available, otherwise no span |
| TS2362/TS2363 arithmetic operand mismatch | offending operand span |
| TS2365 invalid operator | operator or whole expression span, pinned |
| TS2367 no-overlap equality | operator or whole expression span, pinned |
| TS2872/TS2873 truthiness | condition or literal span |
| typescript-rust::duplicate-default-export | default keyword or export statement span, pinned |
| typescript-rust::unsupported-module-syntax | import/export statement span, pinned |
| parser-error | parser-provided best-effort span when available; otherwise no span |

## Notes

- Successful relative imports bind the available type and/or value namespace. Namespace misuse underlines the local usage site, while failed module/export lookups underline the import/export token itself.
- TS2305 should underline the imported specifier name. TS2307 should underline the module specifier string.
- TS2314/TS2315 should stay pinned to the type reference name; v0.59 uses
  these as stable arity diagnostics for generic alias/interface references.
- Missing default exports underline the default-import identifier, and missing
  relative default-import modules underline the module specifier string.
- Missing re-export modules underline the module specifier string. Missing
  re-export members underline the exported specifier name.
- Namespace import property failures underline the property name, while
  namespace import module failures underline the module specifier string.
- Star re-export module failures underline the module specifier string.
- `export * as Foo from ...` stays parser-safe or pinned rather than adding a
  separate span policy in this phase.
- Duplicate generic type parameters use a custom checker diagnostic and should
  underline the repeated name span, not the declaration keyword.
- Call-expression type arguments are parsed for syntax stability, but v0.59
  ignores them in checker flow, so no dedicated diagnostic span is emitted yet.
- Non-relative package-style imports intentionally do not resolve. They either
  emit TS2307 with the module specifier span or a pinned unsupported-module
  diagnostic for parser-safe unsupported syntax.
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
