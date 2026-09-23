export {};
const n = 1;
new n();
const o = { a: 1 };
new o();
class C {}
new C();
declare const anyv: any;
new anyv();
type Ctor = new () => C;
declare const ctor: Ctor;
new ctor();
declare const both: (new () => C) | number;
new both();
