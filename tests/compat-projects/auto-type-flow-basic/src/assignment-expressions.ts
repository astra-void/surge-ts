export {};
declare const c: boolean;
declare function f(): number;
function r2() { let x: number; if ((x = f())) { } const s: number = x; }
function r3() { let a: number, b: string; [a, b] = [1, "s"]; const s: string = b; }
function r4() { let x: number | undefined; x ??= 1; const s: number = x; }
function r5() { let x: number; try { x = f(); } catch { x = 0; } const s: number = x; }
function r6() { let x: string | number; switch (f()) { case 1: x = "a"; break; default: x = 2; } const s: string | number = x; }
function r7() { let x: number; ({ x } = { x: 1 }); const s: number = x; }
function r8() { let x: number; const g = () => { x = 1; }; g(); const s: number = x; }
function r9() { let x: number; x = 1; x += 2; const s: string = x; }
function r10() { let a: number, b: number; a = b = 1; const s: string = b; }
function r11() { let m: RegExpExecArray | null; const re = /a/g; while ((m = re.exec("a"))) { const s: string = m[0]; } }
function r12() { let x: number | undefined; c && (x = 1); const n: number = x; }
