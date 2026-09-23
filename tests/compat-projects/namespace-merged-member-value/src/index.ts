export {};

namespace A {
    export class B { constructor(b: number) {} }
    export namespace B { export const b: number = 0; }

    export function f(): number { return 1; }
    export namespace f { export const x = 1; }
}

new A.B(A.B.b);
const fromCall = A.f();
const member = A.f.x;
A.f.missing;
A.notMember;
