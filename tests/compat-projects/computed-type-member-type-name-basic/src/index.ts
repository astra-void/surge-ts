type Keys = "a" | "b";
type Obj = { [Keys]: string };
type K = "x" | "y";
type Obj2 = { [K]: string };
type Obj3 = { [Keys]: string; other: number };
type NumKeys = 1 | 2; type Obj4 = { [NumKeys]: string };
type Mixed = "a" | number; type Obj5 = { [Mixed]: string };
type Single = "a"; type Obj6 = { [Single]: string };
interface Iface { a: string }
type Obj7 = { [Iface]: string };
interface WithKey { [Keys]: string }
type Obj8 = { [string]: number };
declare const sym: unique symbol;
type Obj9 = { [sym]: string };
const Both = "x"; type Both = "x" | "y";
type Obj10 = { [Both]: string };
enum E { A = "a" }
type Obj11 = { [E.A]: string };
export {}
