// `x instanceof Sub` narrows a non-union subject down to the subclass.
class Base {
  base(): number {
    return 1;
  }
}
class Sub extends Base {
  onlyOnSub(): number {
    return 2;
  }
}
declare const value: Base;
export function narrowed(): number {
  if (value instanceof Sub) {
    return value.onlyOnSub();
  }
  return value.base();
}

// The same holds when the constructor is generic; it narrows to `GenericSub<any>`.
class GenericBase<T> {
  base(): T {
    return null as any;
  }
}
class GenericSub<T> extends GenericBase<T> {
  onlyOnSub(): T {
    return null as any;
  }
}
declare const generic: GenericBase<string>;
export function narrowedGeneric(): string {
  if (generic instanceof GenericSub) {
    return generic.onlyOnSub();
  }
  return generic.base();
}

// An unrelated constructor leaves the subject alone rather than replacing it
// with something it never was.
class Unrelated {
  other(): number {
    return 3;
  }
}
declare const unrelated: Base;
export function untouched(): number {
  if (unrelated instanceof Unrelated) {
    return 0;
  }
  return unrelated.base();
}

// A union subject keeps narrowing by member filtering.
declare const union: Base | Sub;
export function narrowedUnion(): number {
  if (union instanceof Sub) {
    return union.onlyOnSub();
  }
  return union.base();
}
