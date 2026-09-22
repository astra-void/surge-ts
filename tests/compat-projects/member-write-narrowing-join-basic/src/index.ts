export {};

declare const list: string[];
declare const set: Set<string>;

// A write to a property reference narrows it from its *declared* type
// (`getAssignmentReducedType`), even after a guard narrowed it to `undefined`,
// and the join after the `if` sees both edges.
export function objectJoin(o: { c: string[] | undefined }) {
    if (!o.c) {
        o.c = list;
    }
    return o.c.length;
}

export function optionalJoin(o: { c?: string[] }) {
    if (!o.c) {
        o.c = list;
    }
    return o.c.length;
}

export function reassigned(o: { c: string | number }) {
    o.c = 1;
    const n: number = o.c;
    o.c = "x";
    const m: number = o.c;
}

// `this.<member> = v` narrows `this.<member>` the same way.
export class Cache {
    entries: Set<string> | undefined;
    straight() {
        this.entries = set;
        return this.entries.size;
    }
    lazy() {
        if (!this.entries) {
            this.entries = set;
        }
        return this.entries.has("");
    }
    unguarded() {
        return this.entries.size;
    }
}

export class GenericCache<T extends string> {
    entries: Set<T> | undefined;
    constructor(private values: T[]) {}
    has(x: T) {
        if (!this.entries) {
            this.entries = new Set(this.values);
        }
        return this.entries.has(x);
    }
}

// A class merged with a same-named interface is one declaration: its own
// private members are reachable inside it.
interface Wrapper {
    text(): string;
}
export interface Raw<T> extends Wrapper {}
export class Raw<T> {
    constructor(public run: () => T, private sql: string) {}
    text() {
        return this.sql;
    }
}
