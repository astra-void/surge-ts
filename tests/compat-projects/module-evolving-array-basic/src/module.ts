export {};
declare const c: boolean;
const a1 = [];
a1.length;
const r1 = a1;
a1.push(1);
const r2 = a1;

let a2 = [];
for (let i = 0; i < 3; i++) {
  const r3 = a2;
  a2.push("s");
}

const a3 = [];
const f3 = () => a3;
function g3() { return a3; }
class C3 { m() { return a3; } }

let a4 = [];
const f4 = () => a4;
const r4 = a4;

let a5 = [];
a5 = [1];
const r5 = a5;

var a6 = [];
if (c) { a6.push(1); }
const r6 = a6;

const a7 = [];
{ const r7 = a7; }

const a8 = [];
a8[0] = 1;
const r8 = a8;

export const a9 = [];
const r9 = a9;

let a10 = [];
const f10 = () => { a10.push(1); };
const r10 = a10;

const a11 = [];
while (c) {
  const r11 = a11;
}
