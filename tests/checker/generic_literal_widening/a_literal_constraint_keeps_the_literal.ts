// A constraint that asks for a literal keeps it.
declare function lit<T extends string>(v: T): T;
export const kept: "x" = lit("x");
