declare const source: { a: number; b: string; 1: boolean };
declare const either: "a" | "b";
const aKey = "a";

const { [aKey]: fromConst } = source;
const { [either]: fromUnion } = source;
const { "b": fromString } = source;
const { 1: fromNumber } = source;
const { "0": firstChar } = "text";

export const n: number = fromConst;
export const s: string = fromUnion;
export const t: string = fromString;
export const f: boolean = fromNumber;
export const c: string = firstChar;

export function tupleLiteral() {
    var { 0: first, 1: second } = [10, "hello"];
    var first: number;
    var second: string;
    return [first, second];
}
