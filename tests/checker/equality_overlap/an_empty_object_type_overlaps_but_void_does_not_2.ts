// Every non-nullish value is assignable to `{}`, so an empty object type
// compares with anything — but `void` is still a kind of its own.
declare const nothing: void;
declare const count: number;
export const a = nothing === count;
