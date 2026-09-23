export {};
declare function f(): any;
declare const xs: number[];
declare const o: { a: number };
let n = 0;

for (f() of xs) {}
for ((f()) of xs) {}
for (f() in {}) {}

async function g(ys: AsyncIterable<number>) {
  for await (f() of ys) {}
}

for (n of xs) {}
for (o.a of xs) {}
for (const x of xs) {}

const wrong: string = 1;
export { g, wrong };
