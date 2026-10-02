// @strict: true

declare function acceptFunction<T extends Function>(value: T): T;
declare function acceptStringFunction<T extends (value: string) => string>(value: T): T;
declare function identity<T>(value: T): T;

class GenericClass<T> { value!: T; }
declare const untyped: any;
declare const functionValue: Function;
declare const callable: { (value: string): string; label: string };

acceptFunction(GenericClass);
acceptFunction(functionValue);
acceptFunction(untyped);
acceptStringFunction(untyped);
acceptStringFunction(callable);
acceptStringFunction(<T>(value: T) => value);
acceptStringFunction(<T, U = undefined>(value: T, other?: U) => value);

function forward<T extends (value: string) => string, U extends T>(first: T, second: U): void {
  const sameFirst: T = acceptStringFunction(first);
  const sameSecond: U = acceptStringFunction(second);
  const sameIdentity: U = identity(second);
  sameFirst("first");
  sameSecond("second");
  sameIdentity("identity");
}
