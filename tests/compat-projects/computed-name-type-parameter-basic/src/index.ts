declare function foo<T>(): string;
class C<T> {
  [foo<T>()]() {}
  static [foo<T>()]() {}
  [(null as unknown as T) as string]() {}
}
const E = class<U> { [foo<U>()]() {} };
interface I<V> { [foo<V>()]: number; }
class Ok<T> { [((): string => { const g = <T,>(x: T) => foo<T>(); return g(1); })()]() {} }
export {};
