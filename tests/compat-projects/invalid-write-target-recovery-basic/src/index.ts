export {};
declare function value(): number;
declare let optional: { a: { b: any } } | undefined;
let n = 0;
value() = 1;
optional?.a.b = 3;
value()++;
++(n + 1);
(n + 1) = 2;
optional?.a.b++;
value() += 1;
[value()] = [1];
({ a: value() } = { a: 1 });
(value() as any) = 1;
function rest(...values: number[], last: number) {}
function restAgain(...values: string[], last: string) {}
rest([1], 2);
const [...head, tail] = [1, 2];
const { ...others, picked } = { picked: 1 };
let items: number[] = [], item = 0;
[...items, item] = [1, 2];
const wrong: string = n;
