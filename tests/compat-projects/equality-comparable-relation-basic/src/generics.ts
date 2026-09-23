export {};
function unconstrained<T, U>(t: T, u: U, s: string) {
  t === u;
  u === t;
  t === s;
  s === t;
  t === t;
  t === 1;
}
function constrained<T extends string, U extends T, V extends number>(t: T, u: U, v: V) {
  t === u;
  u === t;
  t === "x";
  t === 1;
  t === v;
  v === 1;
}
function unionConstraint<T extends string | number>(t: T) {
  t === 1;
  t === "a";
  t === true;
}
