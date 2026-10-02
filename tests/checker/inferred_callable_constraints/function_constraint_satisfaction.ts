// @strict: false

function acceptFunction<T extends Function>(value: T): T { return value; }
acceptFunction(1);
acceptFunction(() => {}, 1);
acceptFunction(1, () => {});

function acceptStringFunction<T extends (value: string) => string>(value: T): T {
  return value;
}

class RegularClass { value: string; }
class GenericClass<T> { value: T; }
declare const constructor: { new (value: string): string };
declare const genericConstructor: { new <T>(value: T): T };
interface UntypedFunction extends Function { value: string; }
declare const untypedFunction: UntypedFunction;

acceptStringFunction(new Function());
acceptStringFunction((value: string[]) => value);
acceptStringFunction(RegularClass);
acceptStringFunction(constructor);
acceptStringFunction(<T>(value: T) => value);
acceptStringFunction(<T, U>(value: T, other: U) => value);
acceptStringFunction(GenericClass);
acceptStringFunction(genericConstructor);
acceptStringFunction(untypedFunction);

function checkConstrainedParameters<T extends { (): void }, U extends T>(first: T, second: U) {
  acceptStringFunction(first);
  acceptStringFunction(second);
}
