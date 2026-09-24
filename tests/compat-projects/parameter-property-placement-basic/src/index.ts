export class Base {
    m(p: string) {}
}

export class Derived extends Base {
    constructor(public a: number, readonly b: string, private c?: boolean) {
        super();
    }
    override m(override p: string) {}
    n(public x: number) {}
}

export class Overloaded {
    constructor(protected a: number);
    constructor(a: number) {}
}

export function f(private y: number) {}

export interface Shape {
    new (public x: number): Shape;
    m(readonly z: string): void;
}

export type Callback = (protected w: number) => void;

export class Statics {
    constructor(static s: number) {}
}
