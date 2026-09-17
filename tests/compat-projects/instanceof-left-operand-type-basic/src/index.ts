export {}
class C {}
declare const s: string;
declare const n: number;
declare const lit: "a" | "b";
declare const o: object;
declare const u: unknown;
declare const a: any;
declare const maybe: { x: number } | undefined;
declare const union: string | { x: number };

const ok1 = o instanceof C;
const ok2 = u instanceof C;
const ok3 = a instanceof C;
const ok4 = maybe instanceof C;
const ok5 = union instanceof C;
export function ok6<T>(v: T) { return v instanceof C }

const bad1 = s instanceof C;
const bad2 = n instanceof C;
const bad3 = lit instanceof C;
