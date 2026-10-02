// A *generic* group re-resolves its parameter annotations at every call, from
// the one signature the group keeps, which threw the folded parameter union
// away: an argument written for a later overload was reported against the
// first. tanstack-query's `useQuery({ queryKey, queryFn })` — whose first
// overload demands `initialData` — was six false positives from this.
declare function pick<A>(o: { a: A; init: string }): 1;
declare function pick<A>(o: { a: A }): 2;
export const a = pick({ a: 1 });
export const b = pick({ a: 1, init: "x" });
