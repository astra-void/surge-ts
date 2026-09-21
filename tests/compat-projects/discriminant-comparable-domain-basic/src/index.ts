type U = { kind: false, a: string } | { kind: true, b: string } | { kind: string, c: string };
function f10(x: U) {
    if (x.kind === false) { x.a; }
    else if (x.kind === true) { x.b; }
    else { x.c; }
}
function f11(x: U) {
    switch (x.kind) {
        case false: x.a; break;
        case true: x.b; break;
        default: x.c;
    }
}
type V = { kind: "a", a: string } | { kind: number, n: string } | { kind: string, s: string };
function f12(x: V) {
    if (x.kind === "a") { x.a; x.s; x.n; }
    if (x.kind === 1) { x.n; x.a; }
}
type W = { kind?: "a", a: string } | { kind: "b", b: string };
function f13(x: W) {
    if (x.kind === "b") { x.b; } else { x.a; }
    if (x.kind === undefined) { x.a; }
}
interface TA { kind: 'A'; a: number } interface TB { kind: 'B'; b: string }
function f14(val: TA | TB) {
    if (val[`kind`] === 'B') { return val.b; } else { return val.a; }
}
function f15(val: TA | TB) {
    switch (val[`kind`]) { case 'A': return val.a; case 'B': return val.b; }
}
function f16(val: TA | TB) { return val[`nope`]; }
