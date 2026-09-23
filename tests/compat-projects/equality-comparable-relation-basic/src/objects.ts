export {};
class Base { a = ""; }
class Derived extends Base { b = ""; }
class Other { c = ""; }
declare const base: Base;
declare const derived: Derived;
declare const other: Other;
declare const withA: { a: string };
declare const withB: { b: number };
declare const optA: { a?: string };
declare const optB: { b?: number };
declare const empty: {};
declare const obj: object;
declare const callable: { fn(): Base };
declare const newable: { new (): Base };
declare const fnA: (x: string) => void;
declare const fnB: (x: number) => void;
declare const fnC: (x: string) => number;
declare const strArr: string[];
declare const numArr: number[];
declare const mixedArr: (string | number)[];
declare const tup: [string, number];
declare const tup2: [number, string];
declare const roArr: readonly string[];
declare const date: Date;
declare const re: RegExp;
declare const maybeBase: Base | undefined;
declare const baseOrNull: Base | null;
declare const promise: Promise<string>;
declare const str: string;
declare const mapA: Map<string, number>;
declare const mapB: Map<string, string>;

// accepted
base === derived;
derived !== base;
withA == optA;
optA === optB;
empty === withB;
obj === withA;
base === withA;
strArr === mixedArr;
tup === strArr;
tup === mixedArr;
roArr === strArr;
fnA === fnC;
maybeBase === undefined;
baseOrNull == null;
maybeBase === base;
base === baseOrNull;
mapA === mapA;

// rejected
base === other;
withA === withB;
callable === newable;
fnA === fnB;
strArr === numArr;
tup === tup2;
date === re;
promise === str;
base === str;
withA == 1;
mapA === mapB;
