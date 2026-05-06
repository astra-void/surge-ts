# Real Project Compatibility

This crate tracks compatibility in narrow, oracle-backed phases rather than claiming full TypeScript parity.

## Current coverage

- v0.82: project/file visibility hardening, including recursive directory includes and `.tsx` visibility.
- v0.83: parser-safe binding-pattern parameter support for `TS7031` on object binding elements in function and arrow parameters.

## Still out of scope

- Full JSX semantics.
- DOM, Node, and `@types` discovery.
- Physical `lib.d.ts` loading.
- Full package resolution and package runtime exports resolution.
- `baseUrl`, project references, and broader module-resolution heuristics.
- Full callback contextual typing or generic callback inference.
- Full destructuring semantics, including array and rest binding modeling beyond parser safety.

The compatibility target for this phase remains `tsc` profile oracle comparisons on loaded real projects, not native-profile ergonomics.
