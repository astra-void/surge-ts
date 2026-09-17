interface I<T = string, U> {}
export type A<X = 1, Y, Z = 2, W> = [X, Y, Z, W];
export function f<P = number, Q>(p: P, q: Q) {}
export class C<R = 1, S> {}
export const g = <M = 1, N>(m: M, n: N) => 0;
declare function h<A1 = string, B1 = number>(): void;
