declare let c0: new () => number;
declare let c1: new (x: number) => number;
declare let c1s: new (x: string) => number;
declare let cr: new (x: number) => string;
interface Callable { (x: number): string }
interface Constructable { new (x: number): string }
declare let callable: Callable;
declare let constructable: Constructable;
declare let other: { (x: string): string };
declare let plain: { a: number };
class K { constructor(public x: number) {} }
export function f() {
  c0 = c1; c1 = c0; c1 = c1s; c1 = cr;
  callable = other;
  callable = plain;
  constructable = cr;
  constructable = c1;
  constructable = plain;
  const k: new (x: number) => K = K;
  const k2: new (x: string) => K = K;
}
