class Base { a = ""; }
class Derived extends Base { b = ""; }
class Other { c = ""; }

declare const optionalNumber: { b?: number };
declare const optionalString: { b?: string };
declare const requiredNumber: { b: number };
declare const requiredString: { b: string };
declare const optionalBaseMethod: { fn(a?: Base): void };
declare const optionalOtherMethod: { fn(a?: Other): void };
declare const baseMethod: { fn(a: Base): void };
declare const otherMethod: { fn(a: Other): void };
declare const optionalBaseCtor: { new (a?: Base): Base };
declare const optionalOtherCtor: { new (a?: Other): Base };
declare const mixedA: { a: string; b: "y" };
declare const mixedB: { a: "x"; b: string };
declare const nonPrimitive: object;
declare const shaped: { a: string };
declare const optionalA: { a?: string };
declare const base: Base;
declare const derived: Derived;
declare const other: Other;
declare const indexed: { [key: string]: number };
declare const strings: string[];
declare const numbers: number[];
declare const mixed: (string | number)[];
declare const pair: [string, number];
declare const genericA: { fn<T>(t: T): T };
declare const genericB: { fn<T>(t: T[]): T };
declare const takesString: (x: string) => void;
declare const takesNumber: (x: number) => void;
declare const never: never;
declare const text: string;
declare const empty: {};
declare const prefixedA: `a${string}`;
declare const prefixedB: `b${string}`;
declare const big: bigint;
declare const numberOrX: number | "x";
enum Choice { A, B }
enum Named { X = "x", Y = "y" }
declare const choice: Choice;
declare const named: Named;
declare const flag: boolean;

// Comparable in one direction although assignable in neither.
optionalNumber < optionalString;
optionalString >= optionalNumber;
optionalBaseMethod < optionalOtherMethod;
optionalOtherMethod > optionalBaseMethod;
optionalBaseCtor <= optionalOtherCtor;
mixedA < mixedB;
nonPrimitive < shaped;
optionalA < base;
genericA < genericB;
pair < strings;
strings < pair;
pair < mixed;
base < derived;
choice < 1;
named < "x";
named < text;
flag < true;
never < 1;
empty < text;
prefixedA < prefixedB;
prefixedA < "bcd";
big < 1;
function generic<T, U extends T>(t: T, u: U, x: string) {
  t < u;
  u < t;
  t < x;
}

// Not comparable either way.
requiredNumber < requiredString;
baseMethod < otherMethod;
base < other;
indexed < shaped;
strings < numbers;
takesString < takesNumber;
text < 1;
choice < text;
named < 1;
flag < text;
never < "a";
text > never;
nonPrimitive < text;
big < text;
numberOrX < 1;
