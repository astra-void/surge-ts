// A function value is assignable to a callable object target when it matches the
// target's call signature, and not when its parameter is incompatible — so only
// the mismatched call is rejected.
interface Render { (x: number): void; }
declare function take(fn: Render): void;
const ok = (n: number): void => { void n; };
const bad = (s: string): void => { void s; };
take(ok);
take(bad);
