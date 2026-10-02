// An `infer` capture the pattern match cannot line up degrades instead of
// reporting the capture name as an unknown type in the true branch.
export type IntersectOf<U> = (U extends unknown ? (k: U) => void : never) extends (
k: infer I,
) => void
? I
: never;
declare const merged: IntersectOf<{ a: 1 } | { b: 2 }>;
export const use = merged;
