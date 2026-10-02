// Element and item types decide array/tuple overlap; same shape compares, a
// different element type does not.
declare const pair: [number];
declare const strings: string[];
export const a = pair === strings;
