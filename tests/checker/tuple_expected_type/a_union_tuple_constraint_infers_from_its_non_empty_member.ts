// `T extends [A, ...A[]] | []` is the "one or more, or none" signature zod's
// `tuple` uses. The `[]` member only says the argument may also be empty, so the
// elements must be read from the non-empty member; without looking through the
// union the parameter stayed unsolved, landed on the empty tuple, and reported
// every well-formed argument.
type Item = { a: number };
declare const it: Item;
declare function tuple<T extends [Item, ...Item[]] | []>(v: T): T;
export const a = tuple([it, it]);
export const b = tuple([]);
