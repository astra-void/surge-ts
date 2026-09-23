declare const n: number; declare const u: unknown; declare const s: string;
const { ...r1 } = n;
const { ...r2 } = u;
declare const o: { a: number; b: string }; const { a: aa, ...r4 } = o;
function f({ ...r }: number) {}
declare const un: { a: number } | number; const { ...r5 } = un;
declare const nn: { a: number } | null; const { ...r6 } = nn!;
function g<T>(t: T) { const { ...r7 } = t; }
function h<T extends number>(t: T) { const { ...r8 } = t; }
declare const arr: number[]; const { ...r9 } = arr;
export {}
