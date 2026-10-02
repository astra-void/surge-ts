// @filename: core.ts
// A generic interface whose body references an unresolved name degrades; that
// degraded expansion must never be frozen for other consumers, and adding
// more referencing modules must not change the diagnostic surface shape.
export interface Broken<T> { value: T; oops: MissingThing; }
// @filename: use_0.ts
import { Broken } from "./core";
export function probe_0<T>(seed: T, b: Broken<string>): string {
  return b.value;
}
// @filename: use_1.ts
import { Broken } from "./core";
export function probe_1<T>(seed: T, b: Broken<string>): string {
  return b.value;
}
// @filename: use_2.ts
import { Broken } from "./core";
export function probe_2<T>(seed: T, b: Broken<string>): string {
  return b.value;
}
// @filename: use_3.ts
import { Broken } from "./core";
export function probe_3<T>(seed: T, b: Broken<string>): string {
  return b.value;
}
// @filename: use_4.ts
import { Broken } from "./core";
export function probe_4<T>(seed: T, b: Broken<string>): string {
  return b.value;
}
// @filename: use_5.ts
import { Broken } from "./core";
export function probe_5<T>(seed: T, b: Broken<string>): string {
  return b.value;
}
// @filename: use_6.ts
import { Broken } from "./core";
export function probe_6<T>(seed: T, b: Broken<string>): string {
  return b.value;
}
// @filename: use_7.ts
import { Broken } from "./core";
export function probe_7<T>(seed: T, b: Broken<string>): string {
  return b.value;
}
