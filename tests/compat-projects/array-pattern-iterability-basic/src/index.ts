declare const n: number;
declare const o: { a: number };
declare const maybe: number[] | undefined;
declare const pair: [number, string];

export const [a1] = n;
export const [b1, b2] = o;
export const [c1, c2 = 3] = maybe;
export const [[d1]] = [n];
export const [e1, e2] = pair;

function count(): number {
  return 1;
}
export const [f1] = count();

export function local() {
  const [g1] = o;
  return g1;
}
