export function* bare() { yield; }
export function* nullish() { yield undefined; yield null; }
export function* mixed() { yield; yield 1; }
export const expression = function* () { yield; };

declare let sink: any;
declare const strings: Generator<number, void, string>;
declare const nothing: Generator<number, void, undefined>;
declare const flags: Generator<number, void, boolean>;
declare const asyncStrings: AsyncGenerator<number, void, string>;

export function spread() {
    [...strings];
    [...nothing];
    for (sink of strings);
}

export async function iterate() {
    for await (sink of asyncStrings);
}

export function* delegate(): Generator<number, void, boolean> {
    yield* strings;
    yield* flags;
}
