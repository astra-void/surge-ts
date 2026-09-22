type A = { common: string; property1: () => string };
type B = { common: string; property2: () => string };
declare const u: A | B;
function f1() { return u.property1; }
function f2() { return u.commn; }
function f3() { return u.property1(); }
function f4() { return u.commn(); }
function f5(x: A | B | undefined) { return x?.property1; }
function f6() { return u.common; }
declare const arr: string[] | number[];
function f7() { return arr.lenght; }
function f8(v: string | number) { return v.toFixd(); }
function f9(v: string | number) { return v.toFixed(); }
