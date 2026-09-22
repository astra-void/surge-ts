interface O { a: string; b: number; }
type F1 = (x: number) => typeof x;
type F2 = ({ a: alias }: O) => typeof alias;
type F3 = ([first, second]: [string, boolean]) => typeof second;
type F4 = (x: number, y: typeof x) => void;
type F5 = (...rest: string[]) => typeof rest;
type F6 = (x: number) => typeof missing;
declare function g({ a: renamed }: O): typeof renamed;
declare function h(x: number, y: typeof x): typeof y;
declare const f1: F1; declare const f2: F2; declare const f3: F3; declare const f5: F5;
const r1: string = f1(1);
const r2: number = f2({ a: "", b: 1 });
const r3: string = f3(["", true]);
const r5: number = f5("a");
const r6: number = g({ a: "", b: 1 });
const r7: string = h(1, 2);
declare const f4: F4; f4(1, "x");
export {};
