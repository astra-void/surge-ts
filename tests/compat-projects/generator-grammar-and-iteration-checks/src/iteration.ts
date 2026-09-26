class Plain { x = 1 }
declare const syncIterable: Iterable<number>;
declare const numbers: number[];
declare const count: number;
const extraParameter = { async *[Symbol.asyncIterator](_: number) { yield 0; } };

export async function consume() {
    for await (const x of syncIterable) {}
    for await (const x of numbers) {}
    for await (const x of {}) {}
    for await (const x of count) {}
    for await (const x of extraParameter) {}
    for await (const x of new Plain()) {}
}

export function* delegate() {
    yield* {};
    yield* new Plain();
}

declare const inner: Generator<number, symbol, string>;
export function* typedNext(): Generator<number, boolean, string> {
    const received: number = yield 1;
    const result: number = yield* inner;
    return true;
}

export function* untypedYield() {
    const value = yield;
    return value;
}
