export {};

class C {
    prop = "";
}
class D {
    other = 1;
}

// `typeof` narrows a type variable to `T & string` unless its constraint
// already is the tag's type or can never report it.
export function byTypeof<T>(x: string | T, y: T) {
    if (typeof x === "string") {
        const s: string = x;
    }
    if (typeof y === "string") {
        const s: string = y;
        const t: T = y;
        const n: number = y;
        y.length;
    }
}

export function byTypeofConstrained<T extends number, U extends string | number>(x: T, u: U) {
    if (typeof x === "string") {
        const s: string = x;
    }
    if (typeof u === "string") {
        const n: number = u;
    }
}

// `instanceof` keeps a variable whose constraint derives from the class,
// drops it in the false branch, and otherwise narrows it to `T & C`.
export function byInstanceof<T extends C, U extends D>(v: T | string, w: T | U, x: unknown) {
    if (v instanceof C) {
        const t: T = v;
    } else {
        const s: string = v;
    }
    if (w instanceof C) {
        const t: T = w;
    } else {
        const u: U = w;
    }
}

export function byInstanceofUnconstrained<T>(x: T) {
    if (x instanceof C) {
        const t: T = x;
        const c: C = x;
        const d: D = x;
        x.prop;
    }
}

declare function isFunction(value: unknown): value is Function;

export function byPredicate<T>(x: T) {
    if (isFunction(x)) {
        const f: Function = x;
        const t: T = x;
        const s: string = x;
    }
}

// An anonymous object type does not derive from a class, however it is shaped.
class E {
    x: string | undefined;
}
export function anonymousNotDerived<T extends E>(v: T | { x: string }) {
    if (v instanceof E) {
        const t: T = v;
    } else {
        const o: { x: string } = v;
    }
}
