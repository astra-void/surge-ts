// The fold is permissive on parameters only: a property that matches no
// overload of the group must still report. (Which code and span surge picks is
// the permissive fold's, not tsc's TS2769 — only the line agrees.)
declare function pick<A>(o: { a: A; init: string }): 1;
declare function pick<A>(o: { a: A }): 2;
export const a = pick({ a: 1, init: 2 });
