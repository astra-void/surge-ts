namespace A {
    export class B { constructor(b: number) {} bee = 1; }
    export namespace B { export const b: number = 0; }
}
export = A.B;
