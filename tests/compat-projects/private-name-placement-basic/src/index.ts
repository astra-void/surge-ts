declare const o: any;
const r = #x in o;
const lit = { #y: 1 };
interface I { #z: number; }
type T = { #m(): void };
class C { #p = 1; m(q: any) { return #p in q; } }
export {};
