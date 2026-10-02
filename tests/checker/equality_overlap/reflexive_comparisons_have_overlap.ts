// `symbol`, `bigint`, arrays, tuples and named types all compare with
// themselves; the old whitelist had no arm for any of them.
interface Seen { a: number }
declare const s1: Seen;
declare const s2: Seen;
declare const sym1: symbol;
declare const sym2: symbol;
declare const big1: bigint;
declare const big2: bigint;
declare const arr1: string[];
declare const arr2: string[];
declare const tup1: [number, string];
declare const tup2: [number, string];
export const a = s1 === s2;
export const b = sym1 === sym2;
export const c = big1 === big2;
export const d = arr1 === arr2;
export const e = tup1 === tup2;
