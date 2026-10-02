// A destructuring callback parameter is only an error against a written
// `unknown`; a contextual type the checker failed to resolve must stay silent.
declare function accept(fn: (opts: unknown) => void): void;
accept(({ a }) => { console.log(a); });
