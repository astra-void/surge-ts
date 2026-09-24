export function* literal(): Generator<number, { x: "x" }, unknown> {
    yield 1;
    return { x: "x" };
}

export function* mismatch(): Generator<number, string> {
    return 42;
}

export function* defaulted(): Iterator<number> {
    return { anything: true };
}

export async function* awaited(): AsyncGenerator<number, { x: "x" }> {
    return Promise.resolve({ x: "x" } as const);
}

export async function* asyncMismatch(): AsyncGenerator<number, string> {
    return 1;
}

interface State {
    tag: "state";
}
declare function strategy(run: (a: State) => IterableIterator<State | undefined, void>): void;

strategy(function* (state: State) {
    return state;
});
