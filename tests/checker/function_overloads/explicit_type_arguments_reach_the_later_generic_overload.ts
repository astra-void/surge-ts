// Partially supplied explicit type arguments take the same path, with the rest
// filled from the declared defaults.
declare function pick<A = unknown, B = string>(o: { a: A; init: B }): 1;
declare function pick<A = unknown, B = string>(o: { a: A }): 2;
export const a = pick<number>({ a: 1 });
export const b = pick<number, string>({ a: 1 });
