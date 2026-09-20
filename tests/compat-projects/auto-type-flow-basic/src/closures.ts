export {};
declare const c: boolean;
function p1() { let xs = []; xs.push(1); const f = () => xs; const g: string = f(); }
function p2() { const xs = []; xs.push(1); const f = () => xs; }
function p3() { let xs = []; const f = () => xs; xs.push(1); }
function p4() { let x; x = 1; const f = () => x; const g: string = f(); }
function p5() { let x; const f = () => x; x = 1; }
function p6() { let x; const f = () => x; const g: string = f(); }
function p7() { let xs = []; xs.push(1); function f() { return xs; } }
function p8() { let x; function f() { return x; } x = 1; }
function p9() { let xs = []; for (const i of [1, 2]) xs.push(i); const s: string = xs; }
function p10() { let xs = []; if (c) xs.push(1); else xs.push("a"); const s: string = xs; }
function p11() { let xs = []; xs = [1]; xs.push(2); xs.push("a"); }
function p12() { let x = null; x = "a"; const n: number = x; }
function p13() { let xs = []; xs[0] = 1; xs[1] = "a"; const s: string = xs; }
function p14() { let xs = []; xs.length; }
function p15() { let xs = []; return xs; }
function p16() { let xs = []; const ys = xs.map(x => x); }
function p17() { let x; if (c) { x = 1; } else { x = "a"; } const n: number = x; }
function p18() { let x; x.foo; }
function p19() { let x; const n: number = x; }
function p20() { let x = undefined; x = 1; x = "s"; const n: number = x; }
function p21() { let xs = []; xs.push(1); xs = []; const s: string = xs; }
function p22() { let xs = []; xs.push({ a: 1 }); xs.push({ a: 2, b: "x" }); const s: string = xs; }
function p23() { let xs = []; xs.push(...[1, 2]); const s: string = xs; }
function p24() { let xs = []; xs.unshift("a"); const s: string = xs; }
function p25() { let xs = []; while (c) { xs.push(1); } const s: string = xs; }
function p26() { let x; while (c) { x = 1; } const n: string = x; }
function p27() { let xs = []; xs.push(1); const f = function () { return xs; }; }
function p28() { let x; x = 1; x++; const s: string = x; }
function p29() { var xs = []; xs.push(true); const s: string = xs; }
function p30() { let xs = []; const t = xs!; }
