export {};
function f() {}
class C1 extends f {}
const v = 1;
class C2 extends v {}
class C3 extends number {}
class C4 implements string {}
interface I {}
class C5 implements I {}
declare const anyBase: any;
class C6 extends anyBase {}
const obj = { a: 1 };
class C7 extends obj {}
declare const ctor: new () => { z: number };
class C8 extends ctor {}
class C9 extends C5 {}
