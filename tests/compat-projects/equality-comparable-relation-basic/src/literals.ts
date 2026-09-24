export {};
enum E { A, B }
enum S { X = "x", Y = "y" }
enum F { A, B }
declare const e: E;
declare const s: S;
declare const f: F;
declare const ab: "a" | "b";
declare const cd: "c" | "d";
declare const bool: boolean;
declare const str: string;
declare const num: number;
declare const big: bigint;
declare const sym: symbol;
declare const un: unknown;
declare const nev: never;
declare const an: any;
declare const tl: `a${string}`;
declare const numOrStr: number | string;
declare const nullish: null | undefined;
declare const voidish: void;

// accepted
e === E.A;
e === 0;
s === S.X;
s === "x";
ab === "a";
ab === str;
bool === true;
str === "zzz";
num === 1;
un === 1;
nev === "a";
an === 1;
tl === "abc";
tl === str;
numOrStr === 1;
nullish === undefined;
voidish === undefined;
str == null;
1 === 1;
"a" === "a";

// rejected
e === 5;
e === f;
e === "x";
s === "z";
ab === "c";
ab === cd;
bool === "true";
str === 1;
num === "1";
big === 1;
sym === "a";
tl === "b";
1 === 2;
"a" === "b";
true === false;
numOrStr === true;
