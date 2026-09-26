declare const strings: Generator<number, void, string>;
declare const asyncStrings: AsyncGenerator<number, void, string>;

export function spreadStrings() {
    return [...strings];
}

export async function awaitStrings() {
    for await (const x of asyncStrings);
}

export function* delegateStrings(): Generator<number, void, boolean> {
    yield* strings;
}
