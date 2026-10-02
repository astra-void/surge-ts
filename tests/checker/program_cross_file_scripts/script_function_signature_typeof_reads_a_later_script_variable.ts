// @surge-compare: messages
// @filename: a.ts
function f(x: typeof later): void;
function f(x: any) { }
f({ foo: "" });
f({ foo: 1 });
function g(x: typeof nowhere) { }
// @filename: b.ts
var later: { foo: string } = { foo: "" };
