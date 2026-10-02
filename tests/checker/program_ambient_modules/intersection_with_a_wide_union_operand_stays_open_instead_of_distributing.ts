// Past the distribution arity bound the merge is kept single, but the surface
// must stay open — a closed merge would report every union-arm member as an
// excess property.
type W = { a: 1 } | { b: 1 } | { c: 1 } | { d: 1 } | { e: 1 }
  | { f: 1 } | { g: 1 } | { h: 1 } | { i: 1 } | { j: 1 };
type I = { request: string };
export const w: W & I = { a: 1, request: 'r' };
