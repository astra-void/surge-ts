// Overload *selection*. The arguments are checked once, against the permissive
// fold; with their types in hand the return type is the first overload's that
// accepts them, and the fold's when none does. No candidate is re-checked and
// no diagnostic is rolled back, which is what kept the earlier attempt off
// main.
declare function pick(o: { a: number; init: string }): 'first';
declare function pick(o: { a: number }): 'second';
export const second: 'second' = pick({ a: 1 });
export const first: 'first' = pick({ a: 1, init: 'x' });
