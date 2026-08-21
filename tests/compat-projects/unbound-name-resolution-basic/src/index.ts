// An array pattern's rest binding.
declare const xs: string[];
const [head, ...tail] = xs;
export const restUsed: string[] = tail;
export const headUsed: string = head;

// A `declare global` namespace is a value too.
declare global {
  namespace runtime {
    const version: string;
    namespace nested {
      function make(): string;
    }
  }
}
export const version: string = runtime.version;
export const made: string = runtime.nested.make();

// An explicit type argument may name a function-local type.
declare function identity<T>(value: T): T;
export function withLocalTypeArgument(): string {
  type Local = string;
  return identity<Local>('x');
}

// A nested function's parameter default may name an enclosing local.
export function withNestedDefault(): number {
  const fallback = 7;
  function inner(value: number = fallback): number {
    return value;
  }
  return inner();
}

// `...args` written as a destructuring pattern keeps the parameter, and with it
// the method that declares it.
declare const iterator: Iterator<string>;
export const nextResult = iterator.next();
