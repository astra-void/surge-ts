// `if ("p" in x)` narrows the pushed then/else branch scopes, not just the
// symbol tables built for `&&` operands and ternaries.
type A = { input: string; output: string };
type B = { serialize: string };
function f(t: A | B): string {
if ("input" in t) {
return t.input;
} else {
return t.serialize;
}
}
