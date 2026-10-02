// TS2356 is the `++`/`--` operand rule. Unary `+`/`-` coerce, and tsc accepts
// any operand: reporting it here made `+data` on a contextually-typed `string`
// parameter a false positive (tanstack-query's `(data) => [data, +data]`).
declare const s: string;
export const a = +s;
export const b = -s;
