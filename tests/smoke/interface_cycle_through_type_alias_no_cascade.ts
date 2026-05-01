interface A {
  b: B;
}

type B = A;

let value: A = { b: {} };
