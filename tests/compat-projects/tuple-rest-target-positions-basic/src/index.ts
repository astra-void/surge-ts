declare const optionalTail: [string, number?];
declare const single: [string];
declare const headThenNumbers: [string, ...number[]];
declare const holeThenNumbers: [string, number?, ...number[]];
declare const triple: [string, number, boolean];
declare const numbersThenString: [...number[], string];

export const shortSource: [string, number?, ...number[]] = single;
export const restIntoOptional: [string, number?, ...number[]] = headThenNumbers;
export const middleIntoRest: [string, ...(number | boolean)[]] = triple;
export const leadingIntoRest: [...(string | number)[], boolean] = triple;
export const aroundTheRest: [string, ...number[], boolean] = triple;
export const trailingKept: [...(number | boolean)[], string] = numbersThenString;
export const sameShape: [string, number?, ...number[]] = holeThenNumbers;

export const optionalIntoRest: [string, ...number[]] = optionalTail;
export const holeIntoRest: [string, ...number[]] = holeThenNumbers;
export const restIntoRequired: [string, number, ...number[]] = headThenNumbers;
export const middleMismatch: [string, ...boolean[]] = triple;
export const restIntoLeading: [number, ...number[], string] = numbersThenString;
export const tooShort: [string, number, boolean, ...string[]] = single;
