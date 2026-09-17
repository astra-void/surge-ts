export type K = keyof { a: 1 };
export type Char = K[number];
export type S = string[number];
export type L = "abc"[0];
const c: Char = 1;
export type Len = "abc"["length"];
export type U = ("a" | "b")[number];
