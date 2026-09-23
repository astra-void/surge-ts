declare const fixed: [number, boolean, string];
declare const variadic: [number, boolean, ...string[]];
declare const numbers: number[];

(function (a, b, c) { a.toFixed(); b.valueOf(); c.trim(); })(...fixed);
(function (a, b, c) { a.toFixed(); c.trim(); })(...variadic);
(function (a, ...rest) { rest.length; })(...variadic);
((k?) => k)();
((first, second) => second)(1);
((...none) => none.length)();
((value: string, extra) => value)();
export {};
