export function outer(used: number, unused: string, _ignored: boolean) {
    const kept = used + 1;
    const dropped = kept * 2;
    function helper() {
        return helper();
    }
    class Local {}
    interface Shape {}
    enum Kind { A }
    type Alias = string;
    switch (kept) {
        case 1: {
            const inCase = 1;
            break;
        }
    }
    for (const item of [1, 2]) {
        let shadow = item;
        shadow = 2;
    }
    return kept;
}

export namespace Space {
    const hidden = 1;
    export const shown = 2;
    function inner() {}
}

export function generic<T, U>(value: number): number {
    return value;
}

export function partly<T, U>(value: U): U {
    return value;
}

export type Pair<A, B> = [A, A];
export type Unused<_A> = string;

interface Hidden<T> {
    value: string;
}

class Recursive {
    next?: Recursive;
}

enum Colors { Red }

type Mapped<T> = { [K in keyof T]: T[K] };

export type Inferred<T> = T extends Array<infer U> ? string : never;
