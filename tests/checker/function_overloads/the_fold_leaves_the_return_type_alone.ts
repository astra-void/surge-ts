// The kept signature's return survives the fold itself. Widening it to the
// group's union (or to `any`) would degrade every generic group's result;
// which overload's return a call gets is selection's job, below.
declare function pick<A>(o: { a: A; init: string }): { first: A };
declare function pick<A>(o: { a: A }): { second: A };
export const a: { first: number } = pick({ a: 1, init: "x" });
