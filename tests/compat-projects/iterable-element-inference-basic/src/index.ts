declare function take<T>(items: Iterable<T>): T[];
export const taken: string = take([1, 2, 3]);

declare function firstValue<T>(entries: Iterable<readonly [string, T]>): T;
export const value: string = firstValue([["a", 1]] as [string, number][]);

declare function firstKeyed<T>(entries: Iterable<readonly [PropertyKey, T]>): T;
export const keyed: string = firstKeyed([["a", 1]] as [string, number][]);

declare function drained<T>(items: IterableIterator<T>): T;
declare const iterator: IterableIterator<number>;
export const drainedValue: string = drained(iterator);

declare function boxed<T>(items: Array<T>): T;
export const box: string = boxed([true]);
