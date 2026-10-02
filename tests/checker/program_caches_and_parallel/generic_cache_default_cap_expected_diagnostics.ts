// @filename: mod_0.ts
// One fixture module per index: a generic interface exercised at several
// instantiations, one deliberate TS2322, and a cross-file import chain so the
// module binding/import paths (whose preliminary structures are dropped at
// `preliminary_release`) are all live.
export interface RegionBox_0<T> { value: T; }
export type RegionPair_0<T> = { first: T; second: RegionBox_0<T> };
export const ok_0: RegionBox_0<string> = { value: "ok" };
export const bad_0: RegionBox_0<number> = { value: "oops" };
export function use_0(input: RegionPair_0<boolean>): boolean { return input.first; }
// @filename: mod_1.ts
import { ok_0 } from "./mod_0";
export interface RegionBox_1<T> { value: T; }
export type RegionPair_1<T> = { first: T; second: RegionBox_1<T> };
export const ok_1: RegionBox_1<string> = { value: "ok" };
export const bad_1: RegionBox_1<number> = { value: "oops" };
export function use_1(input: RegionPair_1<boolean>): boolean { return input.first; }
export const chained_1: string = ok_0.value;
// @filename: mod_2.ts
import { ok_1 } from "./mod_1";
export interface RegionBox_2<T> { value: T; }
export type RegionPair_2<T> = { first: T; second: RegionBox_2<T> };
export const ok_2: RegionBox_2<string> = { value: "ok" };
export const bad_2: RegionBox_2<number> = { value: "oops" };
export function use_2(input: RegionPair_2<boolean>): boolean { return input.first; }
export const chained_2: string = ok_1.value;
// @filename: mod_3.ts
import { ok_2 } from "./mod_2";
export interface RegionBox_3<T> { value: T; }
export type RegionPair_3<T> = { first: T; second: RegionBox_3<T> };
export const ok_3: RegionBox_3<string> = { value: "ok" };
export const bad_3: RegionBox_3<number> = { value: "oops" };
export function use_3(input: RegionPair_3<boolean>): boolean { return input.first; }
export const chained_3: string = ok_2.value;
// @filename: mod_4.ts
import { ok_3 } from "./mod_3";
export interface RegionBox_4<T> { value: T; }
export type RegionPair_4<T> = { first: T; second: RegionBox_4<T> };
export const ok_4: RegionBox_4<string> = { value: "ok" };
export const bad_4: RegionBox_4<number> = { value: "oops" };
export function use_4(input: RegionPair_4<boolean>): boolean { return input.first; }
export const chained_4: string = ok_3.value;
// @filename: mod_5.ts
import { ok_4 } from "./mod_4";
export interface RegionBox_5<T> { value: T; }
export type RegionPair_5<T> = { first: T; second: RegionBox_5<T> };
export const ok_5: RegionBox_5<string> = { value: "ok" };
export const bad_5: RegionBox_5<number> = { value: "oops" };
export function use_5(input: RegionPair_5<boolean>): boolean { return input.first; }
export const chained_5: string = ok_4.value;
