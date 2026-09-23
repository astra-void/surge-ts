function f() {
    let x: string | number;
    if (typeof x === "string") { x.length; } else { x; }
    x;
    let y: string;
    if (!y) { return; }
    y.length;
    let z: number;
    const q = typeof z === "number" && z + 1;
    let w: string;
    if (w === "a") { w; }
    w;
    let v: string;
    if (v !== undefined) { v; } else { v; }
    let u: boolean;
    if (false) { u; }
    return [q];
}

declare function isText(value: unknown): value is string;
function predicates() {
    let p: string | number;
    if (isText(p)) { p.length; } else { p; }
    let q: string | number;
    const r = isText(q) && q.length;
    let t0: number, t1: number;
    t0 = 1, t1 = t0;
    return [r, t1];
}
