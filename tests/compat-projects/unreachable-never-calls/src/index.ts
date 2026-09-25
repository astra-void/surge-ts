function fail(message?: string): never {
    throw new Error(message);
}

function assert(value: unknown, message?: string): asserts value {
    if (!value) throw new Error(message);
}

function overloaded(): never;
function overloaded(message: string): never;
function overloaded(message?: string) {
    throw new Error(message);
}

declare const annotated: (reason: string) => never;
const inferred = (): never => { throw new Error(); };

namespace Debug {
    export declare function fail(message?: string): never;
    export function check(value: unknown): asserts value {}
}

class Base {
    stop(): never {
        throw new Error();
    }
}

class Derived extends Base {
    halt(): never {
        throw new Error();
    }
    viaThis(x: number) {
        this.halt();
        x;
    }
    viaSuper(x: number) {
        super.stop();
        x;
    }
    viaArrow(x: number) {
        const run = () => {
            this.stop();
            x;
        };
        return run;
    }
}

export function direct(x: number) {
    fail();
    x;
}

export function asserts(x: number) {
    assert(false);
    x;
    x;
}

export function assertsAnd(x: number) {
    assert(false && x > 0);
    x;
}

export function assertsTrue(x: number) {
    assert(true);
    x;
}

export function overloads(x: number) {
    overloaded("stop");
    x;
}

export function typed(x: number) {
    annotated("stop");
    x;
}

export function untyped(x: number) {
    inferred();
    x;
}

export function namespaced(x: number) {
    Debug.fail();
    x;
}

export function namespacedAssert(x: number) {
    ((Debug).check)(false);
    x;
}

export function comma(x: number) {
    x, fail();
    x;
}

export function notStatement(x: number) {
    const y = fail();
    return [x, y];
}

export function shadowed(x: number) {
    const fail = (): never => { throw new Error(); };
    fail();
    x;
}

export function parameter(x: number, stop: () => never) {
    stop();
    x;
}

export { Derived };
