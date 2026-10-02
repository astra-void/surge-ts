// A fresh object literal widens the same way a fresh primitive does:
// `subject({ n: 1 })` binds `T` to `{ n: number }`, so a later `next({ n: 2 })`
// fits. A `const` assertion is not fresh and keeps its literals.
declare function subject<T>(initial: T): { next: (v: T) => void; get: () => T };
export function f() {
const value = subject({ n: 1 });
value.next({ n: 2 });
const n: number = value.get().n;
const kept = subject({ n: 1 } as const);
const one: 1 = kept.get().n;
}
