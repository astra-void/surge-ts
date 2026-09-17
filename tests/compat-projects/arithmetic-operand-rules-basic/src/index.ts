declare const text: string;
declare const count: number;
declare const flag: boolean;
declare const big: bigint;
declare const loose: any;
declare const mixed: number | bigint;

const sameBig: string = big * big;
const bigLiteral: string = big * 2n;
const literalOnly: string = 1n;
const numberTimesBig = count ** big;
const mixedTimes = mixed * 2;
const shiftBig = big >>> 1n;
const anyWithBig: string = loose % big;
const flagsAnd = flag & flag;
const flagsOr = flag | true;
const badLeft = text - count;
const badBoth = text * text;
const ok: number = count * count;
