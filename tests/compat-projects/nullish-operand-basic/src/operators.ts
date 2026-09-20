declare let u: number | undefined;
declare let s: string | undefined;
declare let big: bigint;

export const arithmetic = null * 1;
export const rightUndefined = 1 - undefined;
export const addNull = null + 1;
export const compare = null < 1;
export const negate = -null;
export const key = "k" in null;
export const named = u * 2;
export const namedAdd = u + 1;
export const namedCompare = u < 1;
export const parenthesized = (null) * 2;
export const parenthesizedName = ((u)) * 2;
export const parenthesizedMember = (null).x;

// Concatenation takes a nullish operand, and `any` arithmetic is a number.
export const concatenated: string = s + "x";
export const concatenatedNull = null + "x";
export const bigintMix = big + 1;
export const booleanMix = true + 1;
declare let loose: any;
export const numeric: string = loose * 2;
