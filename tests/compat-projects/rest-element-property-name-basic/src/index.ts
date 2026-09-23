declare const o: { a: number; b: string };
const { ...a: b } = o;
function f({ ...c: d }: { a: number }) {}
const { a: first, ...rest } = o;
export {}
