// An `in` guard composed into an `&&` chain narrows the branch too.
type A = { input: string };
type B = { serialize: string };
function f(t: A | B, ok: boolean): string {
if (ok && "input" in t) {
return t.input;
}
return "";
}
