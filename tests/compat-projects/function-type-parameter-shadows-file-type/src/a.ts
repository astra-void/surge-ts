interface X {
    a: number;
}
type Y = { a: number };
function clash<X>(arg: X) {
    type C = X;
    let c: C = arg;
    type Cond1 = X extends [infer A] ? A : never;
    type Cond2 = X extends [infer A] ? A : never;
    let first: Cond1 = null as any;
    let second: Cond2 = null as any;
    return [c, first, second];
}
function noClash<T>(arg: T) {
    type C = T;
    let c: C = arg;
    return c;
}
function readsFileType(arg: X) {
    type C = X;
    let c: C = arg;
    let wrong: C = { a: "s" };
    return [c, wrong];
}
function localShadows() {
    interface X {
        b: string;
    }
    let v: X = { b: "s" };
    type Y = { b: string };
    let w: Y = { b: "s" };
    type Z = X;
    let z: Z = { b: "s" };
    let wrong: Z = { b: 1 };
    return [v, w, z, wrong];
}
export {};
