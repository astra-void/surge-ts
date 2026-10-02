interface E { data: string } interface IsE { (v: unknown): v is E } declare const isE: IsE; export function f(x: { a: 1 } | E) { if (isE(x)) { const n: number = x.data; } }
