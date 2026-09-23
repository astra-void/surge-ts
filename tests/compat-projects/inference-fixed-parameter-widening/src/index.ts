declare function g8<T>(x: T, f: (x: T) => T): T;
declare function foldLeft<U>(z: U, f: (acc: U, t: boolean) => U): U;
declare function callr<T extends unknown[], U>(args: T, f: (...args: T) => U): U;
declare function pairUp<T>(x: T, f: (x: T) => void): T;
declare const sn: [string, number];
declare function f15(a: string, b: number): string | number;

// A callback parameter typed from its contextual signature fixes the type
// parameters that signature names, and a fixed inference widens its literal
// candidates.
const x11 = g8(1, x => x + 1);
const x11Number: number = x11;
let folded: boolean = foldLeft(true, (acc, t) => acc && t);
const fixedString = pairUp("a", x => {});
let reassigned = fixedString;
reassigned = "b";

// A rest parameter infers from the source's remaining parameters as one
// tuple.
const x31 = callr(sn, f15);
const x31Value: string | number = x31;

const notString: string = g8(1, x => x + 1);
const literalKept: "a" = pairUp("a", x => {});
