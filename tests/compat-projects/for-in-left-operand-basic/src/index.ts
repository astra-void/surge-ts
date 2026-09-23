declare const o: { a: number; b: string };
let n: number; for (n in o) {}
let s: string; for (s in o) {}
let k: "a"; for (k in o) {}
let ab: "a" | "b"; for (ab in o) {}
let any_: any; for (any_ in o) {}
let u: unknown; for (u in o) {}
declare const arr: number[]; let m: number; for (m in arr) {}
function f<T extends object>(t: T) { let x: string; for (x in t) {} let y: number; for (y in t) {} }
let e: {}; for (e in o) {}
let z: number; for (z in {}) {}
export {}
