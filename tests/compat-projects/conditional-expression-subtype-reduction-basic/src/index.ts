// A conditional expression's type drops the branch that is a strict subtype
// of the other.
declare const noArgs: () => void;
declare const optionalArg: (x?: string) => void;
declare const requiredUndefined: (x: string | undefined) => void;
declare const optionalHello: (x?: "hello") => void;
declare const requiredHello: (x: "hello" | undefined) => void;
declare const takesNumber: (x: number) => void;
declare const cond: boolean;

export function fewerParameters() {
    const f = cond ? noArgs : optionalArg;
    f();
    f("hello");
}

export function requiredIsTheStrictSubtype() {
    const f = cond ? requiredUndefined : optionalArg;
    f();
    f("hello");
}

export function narrowerOptional() {
    const f = cond ? requiredUndefined : optionalHello;
    f();
    f("hello");
}

export function narrowerRequired() {
    const f = cond ? requiredHello : optionalArg;
    f();
    f("hello");
}

export function unrelated() {
    const f = cond ? takesNumber : optionalArg;
    f(1);
}

interface Base { a: string }
interface Derived extends Base { b: number }
declare const base: Base;
declare const derived: Derived;

export function derivedIsAbsorbed() {
    const x = cond ? base : derived;
    const y: Derived = x;
    return y;
}

declare const n: number;

export function freshLiteralsStayApart() {
    const o = cond ? { a: n } : { a: n, b: n };
    const t: { a: number; b: number } = o;
    return t;
}

declare const plain: { a: string };
declare const withOptional: { a: string; b?: number };

export function optionalMemberMustBePresent() {
    const w = cond ? plain : withOptional;
    const n: number = w;
    return n;
}
