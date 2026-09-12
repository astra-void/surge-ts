declare function addToStart<T>(items: Array<T>, item: T, max?: number): Array<T>;
declare function addToEnd<T>(items: ReadonlyArray<T>, item: T): Array<T>;
declare function firstOf<T>(items: Array<T>): T;

const items = [1, 2, 3];
const item = 4;

// The array parameter contributes `number`, the bare one the literal `4`; the
// common primitive wins, so `T` is `number` and neither argument is rejected.
export const started: Array<number> = addToStart(items, item);
export const ended: Array<number> = addToEnd(items, item);

// Order does not matter, and a tuple argument infers element-wise too.
const pair: [string, string] = ['a', 'b'];
export const withTuple: Array<string> = addToEnd(pair, 'c');

// A single array argument still drives inference on its own.
export const head: number = firstOf(items);

const labels = ['x', 'y'];
export const badHead: number = firstOf(labels);
