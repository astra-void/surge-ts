# Real Project Compatibility

This crate tracks compatibility in narrow, oracle-backed phases rather than claiming full TypeScript parity.
Its compatibility surfaces are raw measurements, not root-cause classifiers.

## Current coverage

- v0.82: project/file visibility hardening, including recursive directory includes and `.tsx` visibility.
- v0.83: parser-safe binding-pattern parameter support for `TS7031` on object binding elements in function and arrow parameters.
- v0.84.5: deterministic parallel project-checking foundation. This only changes how per-file work is scheduled in project mode; it does not add new semantic, resolver, lib, or declaration behavior.
- v0.84.8: real-source syntax/scope reconciliation fixtures for optional typed parameters, async locals, destructuring locals, nested object shorthand, early returns, type import visibility, and a narrow `TextEncoder` builtin.
- v0.85: generated default-lib foundation from the local TypeScript package, including ambient core and DOM subset loading plus `noLib` disabling.
- v0.86: auth-kit stays at 0 diagnostics while module binding avoids repeated loaded-file scans via canonical identity lookup, and the timing buckets now expose the dominant declaration-collection and export-resolution loops. On the measured auth-kit project, `module_binding` fell from 22.731s to 2.049s and `type_declaration_collection` from 11.041s to 3.743s, with benchmark medians improving from 29.34s to 7.42s at `jobs=1` and from 28.47s to 6.20s at `jobs=4`.

auth-kit currently matches TypeScript with 0 diagnostics under the measured
command set.

## Still out of scope

- Full JSX semantics.
- Full lib.d.ts parity beyond the generated subset.
- Node and `@types` discovery.
- Full package resolution and package runtime exports resolution.
- `baseUrl`, project references, and broader module-resolution heuristics.
- Full callback contextual typing or generic callback inference.
- Full destructuring semantics, including array and rest binding modeling beyond parser safety.

The compatibility target for this phase remains `tsc` profile oracle comparisons on loaded real projects, not native-profile ergonomics. `tsc` remains the default diagnostic profile; `native` is opt-in.
