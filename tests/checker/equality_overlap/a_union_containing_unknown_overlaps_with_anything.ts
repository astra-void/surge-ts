// A union member that degraded to `unknown` must not make the whole comparison
// look disjoint — the trpc `T | typeof marker` sentinel shape.
const marker = Symbol();
export function once<T>(fn: () => T): () => T {
let result: T | typeof marker = marker;
if (result === marker) {
result = fn();
}
return () => result as T;
}
