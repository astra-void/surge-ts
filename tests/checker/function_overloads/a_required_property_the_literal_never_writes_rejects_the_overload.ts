// An object literal's written keys are known from the syntax alone, so an
// overload requiring a property the literal never writes is rejected even
// when the literal's type could not be trusted. tanstack-query's
// `useQuery<string, Error>({ queryKey, queryFn })` was typed by the
// `initialData` overload's result without this, and every narrowing on it was
// wrong.
declare function pick<A>(o: { a: A; init: string; cb?: () => A }): 'first';
declare function pick<A>(o: { a: A; cb?: () => A }): 'second';
export const second: 'second' = pick({ a: 1, cb: () => 1 });
