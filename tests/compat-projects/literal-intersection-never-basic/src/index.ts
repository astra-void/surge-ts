interface Left {
  alpha: 1;
  beta: 2;
}
interface Right {
  gamma: 3;
}

// Disjoint literals have no common inhabitant.
export const disjointKeys: never = null as any as (keyof Left & keyof Right);
export const disjointLiterals: never = null as any as ('a' & 'c');
export const disjointNumbers: never = null as any as (1 & 2);

// The same literal on both sides survives.
export const shared: 'a' = null as any as ('a' & 'a');
export const sharedNumber: 1 = null as any as (1 & 1);

// Two reductions surge still does not do, left out deliberately rather than
// pinned as passing: an overlapping union pair (`('a'|'b') & ('b'|'c')` should
// be `'b'`) and a literal narrowing a primitive (`string & 'a'` should be
// `'a'`). Both predate this change and both still report.

// This is what the reduction is for: a guard written as `extends never`.
type Collides<TKey extends string> = `The property '${TKey}' collides.`;
type Protected<T, W> = keyof T & keyof W extends never ? T & W : Collides<string & keyof T & keyof W>;
declare const merged: Protected<Left, Right>;
export const reachedTrueBranch: number = merged.alpha;
