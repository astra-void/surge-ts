export {};
declare const c: boolean;
declare function f(): number;
declare function g(): string | undefined;
function a1() { let x: number; c ? (x = 1) : (x = 2); const s: string = x; }
function a2() { let x: number; c ? (x = 1) : 0; const n: number = x; }
function a3() { let x: number | undefined; const y = c || (x = 1); const n: number = x; }
function a4() { let x: string | undefined; if (c && (x = g())) { const n: number = x; } }
function a5() { let x: number; if (c || (x = f())) { const n: number = x; } }
function a6() { let x: number | undefined; x ??= f(); const s: string = x; }
function a7() { let x: number | undefined; const y = (x ??= f()); const s: string = y; }
function a8() { let m: string | undefined; while ((m = g()) !== undefined) { const n: number = m; } }
function a9() { let x: number; for (const i of [1]) { if ((x = i) > 0) break; } }
function a10() { let a: number, b: string; ({ a, b } = { a: 1, b: "s" }); const n: string = a; }
function a11() { let a = 0, b = ""; [a, b] = [b, a]; }
function a12() { let x: number; const y: string = (x = 1); }
let top: number;
if ((top = f())) { const s: string = top; }
const topRead: number = top;
