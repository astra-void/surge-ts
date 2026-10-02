// `if (!x) { x = …; }` joins the branch end with the fall-through, so the
// default-an-optional idiom leaves the binding non-nullish afterwards.
declare function make(): number;
export function f(x?: number): number {
if (!x) {
x = make();
}
return x;
}
