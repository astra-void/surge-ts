// tsc intersects the constituents' write types for a union key, so two members
// of different types leave `never`.
// @surge-compare: messages
declare const o: { a: number; b: string };
declare const k: "a" | "b";
export function f() { o[k] = 1; }
