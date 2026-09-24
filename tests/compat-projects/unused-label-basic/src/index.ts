outer: for (let i = 0; i < 1; i++) { break outer; }
unused: for (let i = 0; i < 1; i++) { break; }
lbl: { const x = 1; }
function f() { x: while (true) { break x; } y: while (true) { break; } }
inner: { function g() { inner: { break inner; } } }
cont: for (const k of [1]) { continue cont; }
arrow: { const h = () => { arrow: { break arrow; } }; }
declare const flag: boolean; a: b: for (; flag;) { continue a; }
export {}
