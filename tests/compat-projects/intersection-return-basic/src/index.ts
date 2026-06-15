type A = { a: string };
type B = { b: number };

function make(): A & B {
  return { a: "x", b: 1 };
}

function bad(): A & B {
  return { a: "x" };
}

const value = make();
const a: string = value.a;
const b: number = value.b;
