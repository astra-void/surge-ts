export {};
declare const b: boolean;
declare const u: unknown;
declare const n: string | undefined;
declare const k: string;
const sym: unique symbol = Symbol();
class K {
  [b]() {}
  [u] = 1;
  static [n]() {}
  [k] = 2;
  [sym] = 3;
  ["lit"] = 4;
  [(k as any)] = 5;
  [k + "x"] = 6;
  [missing] = 7;
}
interface I { [k + "x"]: number; [k]: number; }
type T = { [k + "y"]: string };
