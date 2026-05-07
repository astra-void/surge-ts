# Real Project Compatibility

This crate tracks compatibility in narrow, oracle-backed phases rather than claiming full TypeScript parity.

## Current coverage

- v0.82: project/file visibility hardening, including recursive directory includes and `.tsx` visibility.
- v0.83: parser-safe binding-pattern parameter support for `TS7031` on object binding elements in function and arrow parameters.
- v0.84.5: deterministic parallel project-checking foundation. This only changes how per-file work is scheduled in project mode; it does not add new semantic, resolver, lib, or declaration behavior.

## Still out of scope

- Full JSX semantics.
- DOM, Node, and `@types` discovery.
- Physical `lib.d.ts` loading.
- Full package resolution and package runtime exports resolution.
- `baseUrl`, project references, and broader module-resolution heuristics.
- Full callback contextual typing or generic callback inference.
- Full destructuring semantics, including array and rest binding modeling beyond parser safety.

The compatibility target for this phase remains `tsc` profile oracle comparisons on loaded real projects, not native-profile ergonomics. `tsc` remains the default diagnostic profile; `native` is opt-in.
