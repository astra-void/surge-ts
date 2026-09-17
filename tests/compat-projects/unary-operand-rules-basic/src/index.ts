declare const key: symbol;
declare const big: bigint;
declare const maybe: number | undefined;
declare const text: string;

const negatedSymbol = -key;
const plusSymbol = +key;
const flippedSymbol = ~key;
const plusBigInt = +big;
const negatedMaybe = -maybe;
const flippedText: string = ~text;
const negatedBig: string = -big;

const coerced: number = +text;
const flippedNumber: number = ~5;
