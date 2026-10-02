declare function pick<A>(o: { a: A; init: string }): { first: A };
declare function pick<A>(o: { a: A }): { second: A };
export const inferred: { second: number } = pick({ a: 1 });
export const explicit: { second: number } = pick<number>({ a: 1 });
export const first: { first: number } = pick({ a: 1, init: 'x' });
