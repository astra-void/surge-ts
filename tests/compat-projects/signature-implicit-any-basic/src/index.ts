interface NG { new <T>(n: string); }
declare var ng: NG;
new ng('');
interface A { f: { new (x: number); } }
declare var b: A;
new b.f(1);
interface J { bar(x = 1): void; (x = 1); baz(y); qux(): void; }
declare var j: J;
j.bar(1); j(); j.baz(1); j.qux();
type Ctor = new (x) => void;
declare var c: Ctor;
new c(1);
new ng(1);
export {};
