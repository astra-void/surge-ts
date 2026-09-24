interface i09<T, U, V = number> { }
type i09t00 = i09;
type i09t01 = i09<1>;
type i09t02 = i09<1, 2>;
type i09t03 = i09<1, 2, 3>;
type i09t04 = i09<1, 2, 3, 4>;

type A2<T, U = string> = { t: T; u: U };
type a0 = A2;
type a1 = A2<number>;
type a3 = A2<number, string, boolean>;

interface Req<T, U> { }
type r1 = Req<number>;
type r2 = Req<number, string>;
