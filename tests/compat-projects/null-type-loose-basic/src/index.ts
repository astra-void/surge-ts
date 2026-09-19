export {};
var a: string = null;
let u: undefined = null;
let n: null = undefined;
let o: { x: string } = { x: null };
function f(): number { return null; }
declare let x: string | null;
const len: number = x.length;
const obj = { p: null };
obj.p = 1;
const arr = [null];
arr.push(1);
const c = null;
let c2: number = c;
let bad: number = "s";
