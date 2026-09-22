export {};
var a: string = null;
var b: string;
b = null;
function f(): string { return null; }
class K { m(): number { return null; } p: string = null; }
let u: undefined = null;
let n: null = undefined;
let v: void = null;
let ok1: string | null = null;
let ok2: any = null;
let ok3: unknown = null;
let o: { x: string } = { x: null };
let arr: string[] = [null];
function g(x: number) {}
g(null);
const c = null;
let c2: number = c;
let e = null;
e = 1;
let t: [string, number] = [null, 1];
declare let sn: string | null;
let s1: string = sn;
let s2: string = sn!;
let s3: string = sn ?? "d";
const obj = { p: null };
obj.p = 1;
const w = [1, null];
const wv: number[] = w;
declare let target: string | null;
target = 1;
declare let both: string | null | undefined;
both = true;
const pair: number[] = [null, undefined];
