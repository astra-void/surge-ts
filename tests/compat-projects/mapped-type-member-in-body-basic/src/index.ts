declare const key: "a";
export interface I { a: 1; [k in "a"]: 1 }
export interface I2 { [k in "b"]: 1; b: 2 }
export type T = { b: 2; [k in "a"]: 1 };
export type T2 = { m(): void; [key in {}]: 1 };
export class C { c = 1; [k in "a"] = 1 }
export class D { m() {} [k in "a"] = "x" }
export class E { [k in "a"] = 1; [j in "b"] = 2 }
export class F { accessor x = 1; accessor [k in "a"] = 1 }
export class G { static { } [k in "a"] = 1 }
export class J { get [k in "a"]() { return 1; } }
export class K { #p = 1; [#p in {}] = 1 }
export class Ok { [key] = 1; ["x"] = 2 }
export type Mapped = { [K in "a"]: K };
