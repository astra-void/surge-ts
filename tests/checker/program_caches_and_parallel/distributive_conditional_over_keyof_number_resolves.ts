// @filename: util.ts
// A member that resolved to the `unknown` degradation sentinel (here via
// `keyof` of a non-object) must get the same "cannot decide" treatment as a
// syntactic sentinel: no branch is selected, no capture goes unbound, and the
// open result stays diagnostic-free.
// `keyof 5` is `keyof Number`, a union of method names, so `MakeRO`
// resolves to string literals that are not assignable to `number`.
export type MakeRO<T> = T extends Map<infer K, infer V>
  ? ReadonlyMap<K, V>
  : Readonly<T>;
// @filename: use.ts
import { MakeRO } from "./util";
type Mystery = keyof 5;
declare const m: MakeRO<Mystery>;
export const ok: number = m;
