// @filename: blowup.ts
// Nested distributive conditionals multiply union widths (20^5 = 3.2M branch
// resolutions here). The per-root expansion budget must degrade the runaway
// alias to `unknown` instead of hanging or exhausting memory; without it this
// test does not terminate in any reasonable time.
type U = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20;
type Cross<A, B, C, D, E> = A extends any
? B extends any
? C extends any
? D extends any
? E extends any
? [A, B, C, D, E]
: never : never : never : never : never;
type Boom = Cross<U, U, U, U, U>;
export const marker: number = 1;
