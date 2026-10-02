// Two object candidates for one parameter infer the union of them, as tsc
// does, so the second argument is not checked against the first. A union
// parameter (`b: T | undefined`) takes the same path: an argument matching no
// structured member is still an answer for the naked one.
declare function eq<T extends Record<string, any>>(a: T, b: T | undefined): boolean;
export const same = eq({ a: 1 }, { a: 2 });
export const wider = eq({ a: 1 }, { a: 1, b: 2 });
