// A written mapped type takes the same rule as the built-in `Record` — the
// physical lib declares `Record<K, T>` as `{ [P in K]: T }`, so only this path
// runs there, and `number`/`symbol` keys collapsed the whole type to the
// sentinel.
type ByKey<K extends string | number> = { [P in K]: boolean };
declare const byString: ByKey<string>;
declare const byNumber: ByKey<number>;
export const a: boolean = byString["k"];
export const b: boolean = byNumber[1];
