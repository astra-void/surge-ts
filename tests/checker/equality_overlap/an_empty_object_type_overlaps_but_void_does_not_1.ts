// Every non-nullish value is assignable to `{}`, so an empty object type
// compares with anything — but `void` is still a kind of its own.
declare const empty: {};
declare const count: number;
declare const fn: (n: number) => void;
export const a = empty === count;
export const b = empty === fn;
