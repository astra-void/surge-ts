declare const running: boolean;
declare const items: number[];
const limit: number = 3;

// Module-scope statements other than declarations are checked like a body's.
for (let index = 0; index < limit; index++) {
  const counted: number = "count";
}
for (const item of items) {
  const label: string = item;
}
for (const key in { a: 1 }) {
  const numeric: number = key;
}
while (running) {
  const looped: number = "loop";
}
do {
  const once: number = "once";
} while (running);
{
  const scoped: number = "scoped";
}
switch (limit) {
  case 3: {
    const matched: number = "three";
    break;
  }
}
try {
  const attempted: number = "try";
} catch {
  const recovered: number = "catch";
}
outer: for (const item of items) {
  const labelled: string = item;
  break outer;
}

// A module-scope binding stays visible inside.
for (const item of items) {
  const bounded: number = item + limit;
}
