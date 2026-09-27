export {};

// A module-local declaration named like a lib type the lib's own declarations
// use. `Iterator.next` returns the lib's `IteratorResult`, whose members come
// from the lib's `IteratorYieldResult`, not from this one.
interface IteratorYieldResult<T> {
  bogus: T;
  done?: false;
}

declare const iterator: Iterator<number, string>;
const result = iterator.next();
if (!result.done) {
  const value: number = result.value;
  const wrong: string = result.value;
  const missing: number = result.bogus;
}
