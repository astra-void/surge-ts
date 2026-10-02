// A block-local `var` that shadows an outer binding restores the outer one at
// the *function* boundary, not at the block's.
const shadowed: string = "outer";
export function f() {
{
var shadowed = 1;
}
return shadowed;
}
export const outer: string = shadowed;
