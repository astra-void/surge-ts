interface Weak {
  a?: number;
  b?: string;
}

declare const count: number;
export const fromPrimitive: Weak = count;

declare function make(): { c: 1 };
export const fromFunction: Weak = make;

declare function makeWeak(): { a: 1 };
export const fromCallable: Weak = makeWeak;

declare const unrelated: { c: 1 };
export const fromObject: Weak = unrelated;

export class ImplementsWeak implements Weak {
  c = 1;
}

export class Base {
  x = 1;
}

export class ImplementsClass implements Base {}

export class EmptyImplementsWeak implements Weak {}
