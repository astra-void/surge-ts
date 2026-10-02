// @filename: core.ts
// Two semantically different argument tuples of the same declaration must not
// collide: `Internals<number, string>` has `out: number`, so returning it as
// `string` is a genuine mismatch that must be reported even though
// `Internals<string, number>` was expanded (and possibly cached) first.
export interface Internals<O, I> {
  out: O;
  inp: I;
  parse(value: I): O;
}
// @filename: use_0.ts
import { Internals } from "./core";
export function pick_0<T>(seed: T, internals: Internals<string, number>): string {
  return internals.out;
}
export function feed_0<T>(seed: T, internals: Internals<string, number>): number {
  return internals.inp;
}
export const bad_0: string = 0;
// @filename: use_1.ts
import { Internals } from "./core";
export function pick_1<T>(seed: T, internals: Internals<string, number>): string {
  return internals.out;
}
export function feed_1<T>(seed: T, internals: Internals<string, number>): number {
  return internals.inp;
}
export const bad_1: string = 1;
// @filename: flip.ts
import { Internals } from "./core";
export function flip<T>(seed: T, internals: Internals<number, string>): string {
  return internals.out;
}
