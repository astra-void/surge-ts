export { }

namespace A {
    export const y = 34;
    export interface y { s: string }
    export namespace N {
        export interface I { i: number }
    }
}

declare global {
    export import x = A.y;
    export import n = A.N;
}

const m: number = x;
let s: x = { s: "" };
let t: x = { s: 1 };
let i: n.I = { i: "" };
let u: n = 1;
