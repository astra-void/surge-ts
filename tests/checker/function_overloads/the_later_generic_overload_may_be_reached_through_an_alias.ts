type Defined<A> = { a: A; init: string };
type Undefined<A> = { a: A };
declare function pick<A>(o: Defined<A>): 1;
declare function pick<A>(o: Undefined<A>): 2;
export const a = pick({ a: 1 });
