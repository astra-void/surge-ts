// @filename: a.ts
// Same declaration name and same argument tuple in two different files with
// different shapes must not collide (the key includes the declaring file).
export interface Shape<T> { tag: string; value: T; }
export function useA<X>(seed: X, s: Shape<number>): string { return s.tag; }
// @filename: b.ts
export interface Shape<T> { tag: number; value: T; }
export function useB<X>(seed: X, s: Shape<number>): string { return s.tag; }
