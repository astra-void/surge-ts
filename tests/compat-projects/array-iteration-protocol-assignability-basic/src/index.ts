declare const numbers: number[];
declare const pairs: [string, number][];
declare const text: string;
declare const tuple: [number, number];

export const iterable: Iterable<number> = numbers;
export const iterableOfTuples: Iterable<readonly [string, number]> = pairs;
export const iterableOfChars: Iterable<string> = text;
export const iterableTuple: Iterable<number> = tuple;
export const arrayLike: ArrayLike<number> = numbers;
export const arrayLikeString: ArrayLike<string> = text;
export const concatArray: ConcatArray<number> = numbers;
export const lengthOnly: { length: number } = numbers;

export const wrongElement: Iterable<string> = numbers;

declare const count: number;
export const primitiveIsNotIterable: Iterable<number> = count;
