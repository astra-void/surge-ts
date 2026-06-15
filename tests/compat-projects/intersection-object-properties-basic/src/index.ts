type A = { a: string };
type B = { b: number };
type AB = A & B;

const ok: AB = { a: "x", b: 1 };
const missingA: AB = { b: 1 };
const missingB: AB = { a: "x" };
const wrongA: AB = { a: 123, b: 1 };

const a: string = ok.a;
const b: number = ok.b;
const wrongRead: boolean = ok.a;
