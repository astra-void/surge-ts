// The fall-through of an early-returning `in` guard sees the complement, not the
// matching member.
type A = { input: string; output: string };
type B = { serialize: string };
function f(t: A | B): A {
if ("input" in t) {
return t;
}
return { input: t.serialize, output: t.serialize };
}
