// `(A | B) & C` is `(A & C) | (B & C)`. The object merge only reads
// `Type::Object` operands, so an undistributed union contributed nothing and
// every member of A/B was reported as an excess property.
type U = { data: number; error: undefined } | { data: undefined; error: string };
type I = { request: string };
export const c: U & I = { data: undefined, error: 'x', request: 'r' };
