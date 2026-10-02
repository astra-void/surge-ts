// @filename: f1.ts
export class A {}
// @filename: f2.ts
export class B {
    n!: number;
}
// @filename: f3.ts
import { A } from "./f1";
import { B as OuterB } from "./f2";
const outer: OuterB = { n: 1 };

namespace N {
    export interface Ifc { a: number }
}

declare module "./f1" {
    import { B } from "./f2";
    import I = N.Ifc;
    interface A {
        foo(): B;
        bar(): I;
    }
}
// @filename: f4.ts
import { A } from "./f1";
import "./f3";

declare let a: A;
let b = a.foo().n;
