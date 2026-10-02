// @filename: src/index.ts
// `declare const w: Win & typeof globalThis` (the lib shape of `window`/`self`)
// resolves `typeof globalThis` before the global object symbol is installed.
// Treating that miss as a clean `unknown` plus the `T & unknown ⇒ T`
// simplification keeps `w` typed as `Win`, so member access is checked against
// it — `w.bar` is `string`. The earlier behaviour emitted a (suppressed) TS2304
// and poisoned `w` to `unknown`, silently dropping the member check; an eager
// re-merge instead corrupted the shared `Win` apparent type.
export const ok: string = w.bar;
export const bad: number = w.bar;
// @filename: types/globals.d.ts
interface Win { bar: string; }
declare const w: Win & typeof globalThis;
