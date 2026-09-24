export {};
interface A { a: string }
interface B { b: number }
interface C { c: boolean }
interface I1 { p1: number }
interface I2 extends I1 { p2: number }
interface I3 { p3: number }
declare const ab: A & B;
declare const bc: B & C;
declare const a: A;
declare const c: C;
declare const y: I1 & I3;
declare const z: I2;
declare const wide: { a: string; b: number };
declare const conflicting: { a: number };
declare const branded: string & { __brand: "x" };
declare const str: string;
declare const num: number;
declare const lit: "a";
declare const abOrC: (A & B) | C;
declare const impossible: string & number;

// accepted
ab === a;
a === ab;
ab === wide;
wide === ab;
branded === str;
str === branded;
branded === lit;
abOrC === c;
abOrC === a;
impossible === str;
function generic<T>(t: T & string, s: string) {
  t === s;
  s === t;
}

// rejected
ab === c;
c === ab;
ab === bc;
bc === ab;
y === z;
z === y;
y !== z;
y == z;
ab === conflicting;
branded === num;
num === branded;
