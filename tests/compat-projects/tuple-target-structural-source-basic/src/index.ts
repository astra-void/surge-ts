interface StrNum extends Array<string | number> {
  0: string;
  1: number;
  length: 2;
}
interface NumArr extends Array<number> {
  extra: string;
}

declare const strNum: StrNum;
declare const lookalike: { 0: string; 1: number; length: 2 };
declare const match: RegExpMatchArray;
declare const numArr: NumArr;

export const pair: [string, number] = strNum;
export const frozenPair: readonly [string, number] = strNum;
export const array: (string | number)[] = strNum;
export const frozenArray: readonly (string | number)[] = strNum;
export const strings: string[] = match;
export const numbers: number[] = numArr;
export const fromTuple: StrNum = ["a", 1] as [string, number];
export const eitherTuple: [number] | [string, number] = strNum;
export const eitherArray: number[] | (string | number)[] = strNum;

export const missingThird: [number, number, number] = strNum;
export const wrongElements: [number] = strNum;
export const notAnArray: [string, number] = lookalike;
export const restTarget: [string, ...number[]] = strNum;
export const wrongArray: number[] = strNum;
export const wrongLength: [string] = match;
export const missingFirst: [number] = numArr;
export const missingTwo: [number, number] = numArr;
export const tooLong: StrNum = ["a", 1, 2] as [string, number, number];
export const neitherTuple: [number] | [string] = strNum;
