export {};

declare function nothing(): void;
function conditions() {
  if (nothing()) {
  }
  const a = nothing() ? 1 : 2;
  while (nothing()) {}
  const b = !nothing();
  const c = nothing() && 1;
  for (; nothing(); ) {}
  do {} while (nothing());
  if (nothing() || 1) {
  }
  const d = nothing() ?? 1;
}

declare const value: object;
interface Plain {
  a: number;
}
declare const plain: Plain;
declare const callable: Function;
declare const custom: { [Symbol.hasInstance](v: unknown): boolean };
class Klass {}
value instanceof plain;
value instanceof callable;
value instanceof custom;
value instanceof Klass;
value instanceof Date;
value instanceof "text";
value instanceof ({} as { new (): object });

declare const flag: boolean;
declare const bag: object;
declare const maybe: string | undefined;
declare const unknownKey: unknown;
declare const literal: "a" | 1;
declare const template: `a${string}`;
enum Keys {
  A,
}
const k1 = { [flag]: 1 };
const k2 = { [bag]: 1 };
const k3 = { [maybe]: 1 };
const k4 = { [unknownKey]: 1 };
const k5 = { [literal]: 1, [Keys.A]: 2, [template]: 3, [1 as any]: 4 };
