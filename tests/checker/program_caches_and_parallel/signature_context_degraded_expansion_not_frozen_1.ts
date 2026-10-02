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
