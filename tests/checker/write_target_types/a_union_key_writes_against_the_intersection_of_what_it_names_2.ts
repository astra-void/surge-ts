// tsc intersects the constituents' write types for a union key, so two members
// of different types leave `never`.
declare const o: { a: number; b: number };
declare const k: "a" | "b";
export function f() { o[k] = 1; }
