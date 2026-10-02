// An array-typed rest parameter keeps its element-wise behaviour: every position
// is the element type, and no positional expansion happens.
// @noImplicitAny: true
declare function run(f: (...args: number[]) => void): void;
run((first, second) => {
const a: number = first;
const b: number = second;
void a;
void b;
});
