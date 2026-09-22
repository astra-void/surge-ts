export {};

// tsc relates a type variable through its constraint (`unknown` when it has
// none), and a type parameter target admits only itself, `never`, `any`, and
// a variable whose constraint leads to it.
export function variables<T, U extends T, V extends string>(t: T, u: U, v: V, n: number) {
    const a: T = u;
    const b: U = t;
    const c: string = v;
    const d: number = v;
    const e: T = n;
    const f: {} = t;
    const g: unknown = t;
    const h: T = null;
    const i: V = "x";
    const j: string = t;
    const k: T | undefined = t;
    const l: {} | null | undefined = t;
    const m: {} | null = t;
    return [a, b, c, d, e, f, g, h, i, j, k, l, m];
}

export function arrays<T>(xs: T[], ys: string[], fn: () => T) {
    const a: T[] = xs;
    const b: string[] = xs;
    const c: T[] = ys;
    const d: () => T = fn;
    const e: () => string = fn;
    return [a, b, c, d, e];
}

export function intersections<T>(x: T, y: NonNullable<T>) {
    x = y;
    y = x;
}

export function constrained<T extends string | undefined>(x: T, y: NonNullable<T>) {
    const s: string = y;
}

export class Box<T> {
    value!: T;
    set(x: string) {
        this.value = x;
    }
    get(): T {
        return this.value;
    }
    copy(other: Box<T>) {
        this.value = other.value;
    }
    method<U>(u: U): T {
        return u;
    }
}

// A constraint surge cannot model leaves the variable unjudged rather than
// reading it as `unknown`.
export function keys<T, K extends keyof T>(k: K) {
    const s: string | number | symbol = k;
    return s;
}
