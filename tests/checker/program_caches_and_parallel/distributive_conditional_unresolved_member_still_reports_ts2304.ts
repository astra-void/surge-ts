// @surge-compare: messages
// @filename: util.ts
// Negative control: a genuinely unresolved name in the instantiation still
// reports its TS2304 — the `any`/sentinel guards must not swallow real
// resolution errors.
export type MakeRO<T> = T extends Map<infer K, infer V>
  ? ReadonlyMap<K, V>
  : Readonly<T>;
// @filename: use.ts
import { MakeRO } from "./util";
export type Broken = MakeRO<Missing>;
