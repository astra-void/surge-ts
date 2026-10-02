// @filename: util.ts
// An `any` member distributed into the conditional must degrade to an open
// `any` (the same rule the non-distributive path applies), not select the
// true branch with its `infer` captures unbound — which resolved
// `ReadonlyMap<K, V>` with `K`/`V` as unresolvable type names (surge-only
// TS2304s on zod v3's `MakeReadonly`) and silently degraded every enclosing
// interface expansion.
export type MakeRO<T> = T extends Map<infer K, infer V>
  ? ReadonlyMap<K, V>
  : Readonly<T>;
// @filename: use.ts
import { MakeRO } from "./util";
export interface Holder<T> { value: MakeRO<T>; }
export function go<T>(seed: T, h: Holder<any>): void {
  const v = h.value;
}
