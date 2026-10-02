// `!("p" in x)` narrows the opposite branch.
type A = { input: string };
type B = { serialize: string };
function f(t: A | B): string {
if (!("input" in t)) {
return t.serialize;
}
return t.input;
}
