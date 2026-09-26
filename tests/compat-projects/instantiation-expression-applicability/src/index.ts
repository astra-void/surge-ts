declare function pair<T, U>(first: T, second: U): [T, U];
declare function single<T>(value: T): T;
declare function single<T>(value: T, count: number): T;
declare const record: { x: string; y: string };

export const pairOfStrings = pair<string, string>;
export const singleString = single<string>;
export const tooMany = single<string, number>;
export const onRecord = record<string>;

export type PairOfNumbers = typeof pair<number, number>;
export type TooFew = typeof pair<number>;
