interface A { a: number }
interface B { b: string }
interface Shared { a: number; shared: string }
interface AlsoShared { b: string; shared: string }

export function weakMembers(x: Partial<A> | Partial<B>) {
    x = {} as A;
    x.a;
}

export function mappedUnion(x: Partial<A | B>) {
    x = x as A;
    x.a;
}

export function commonProperty(x: Partial<Shared> | Partial<AlsoShared>) {
    x = {} as Shared;
    x.a;
}

enum E { a, b }
declare const anything: any;

export function compoundKeepsBaseType(x: E, s: string | undefined) {
    x += anything;
    x += null;
    s += "x";
    s.length;
}

export function compoundWidensLiteral() {
    let n = 1;
    n += 1;
    const one: 1 = n;
    return one;
}

class SQL {
    queryChunks: string[] = [];
}
class Column {
    indexConfig: { order?: string } = {};
    defaultConfig = { order: "asc" };
}
declare function is<T>(value: unknown, type: new (...args: any[]) => T): value is T;

export function build(columns: Partial<Column | SQL>[]) {
    return columns.map((it) => {
        if (is(it, SQL)) {
            return it;
        }
        it = it as Column;
        it.indexConfig = JSON.parse(JSON.stringify(it.defaultConfig));
        return it;
    });
}
