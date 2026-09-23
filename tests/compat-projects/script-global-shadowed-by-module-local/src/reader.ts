import { v3 } from "./local";
export const t1: string = v1;
export const t2: number = v2;
export const t3: boolean = v3;
export var v4 = { a: true, b: NaN };
let shared = 1;
export const t4: number = shared;
export function readGlobal(): number {
    return v2;
}
export const wrong: string = v2;
export function wrongInBody(): string {
    return v2;
}
