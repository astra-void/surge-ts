let literal: "a" = "a";
literal = "b";
literal = ("b");

let count: number = 0;
count += "s";

let shape: { a: number } = { a: 1 };
shape = {};
shape = { a: "s" };

declare let holder: { p: number };
holder.p = "s";

function local() {
  let inner: number = 1;
  inner = "s" as string;
  let innerShape: { a: number } = { a: 1 };
  innerShape = {};
}
