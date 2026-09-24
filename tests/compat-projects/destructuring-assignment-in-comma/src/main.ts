declare function f(): void;

export function leading() {
    let a: number;
    [a] = [1], f();
    return a;
}

export function trailing() {
    let b: string;
    f(), { b } = { b: "x" };
    return b.length;
}

export function neverAssigned() {
    let c: number;
    f(), c;
    return c;
}
