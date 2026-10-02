// @filename: util.ts
// The clean concrete instantiation of the same shape: a real `Map` member
// selects the true branch, binds `K`/`V` from the nominal reference, and the
// resulting `ReadonlyMap<string, number>` keeps checking members (`size` is a
// `number`, so the anchor assignment must still report TS2322).
export type MakeRO<T> = T extends Map<infer K, infer V>
  ? ReadonlyMap<K, V>
  : Readonly<T>;
// @filename: use.ts
import { MakeRO } from "./util";
type RO = MakeRO<Map<string, number>>;
declare const ro: RO;
export const bad: string = ro.size;
