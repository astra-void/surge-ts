// @filename: src/index.ts
// tsc merges the two ambient `declare function getName` declarations as an
// overload set (NOT a duplicate implementation): it reports no TS2393 and
// instead a TS2322 at the call site once the `number` overload is selected.
// surge no longer emits the false TS2393 here. It does not yet build a true
// overload set (it keeps the first signature, so `getName()` stays `string`
// and the TS2322 is under-reported) — a separate overload-merging limitation,
// tracked distinctly from the duplicate-implementation policy this pins.
let ok: string = getName();
// @filename: types/a.d.ts
declare function getName(): string;
// @filename: types/b.d.ts
declare function getName(): number;
