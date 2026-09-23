import { fromLib, fromLib2, nested } from "user";
export const a: number = fromLib.y;
export const b: string = fromLib2;
export const c: string = fromLib2.v;
export const d: number = fromLib2.f();
export const e: string = nested.i;
fromLib2.nope;
