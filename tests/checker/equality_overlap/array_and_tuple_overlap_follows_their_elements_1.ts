// Element and item types decide array/tuple overlap; same shape compares, a
// different element type does not.
declare const strings: string[];
declare const numbers: number[];
export const a = strings === numbers;
