declare function same<T>(a: T, b: T, c: T): T;
declare function each<T>(a: T, b: T, c: T): void;
declare const anything: any;

// An `any` argument is an inference candidate, and every candidate is a
// subtype of `any`: the parameter is `any` wherever the argument stands.
const first = same(anything, 7, 4);
const middle = same(7, anything, 4);
const last = same(7, 4, anything);
const firstIsAny: string = first;
const middleIsAny: string = middle;
const lastIsAny: string = last;
each("", 1, anything);
each(anything, "", 1);

// Without it the candidates compete as before.
let numbers = same(7, 8, 4);
const numbersAreNumber: string = numbers;
each("", 1, 2);
