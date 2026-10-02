// `subject(1)` is `{ next: (v: number) => void }`, so a later `next(2)` fits.
declare function subject<T>(initial: T): { next: (v: T) => void; get: () => T };
export function f() {
const value = subject(1);
value.next(2);
const s = subject("a");
s.next("b");
const b = subject(true);
b.next(false);
}
