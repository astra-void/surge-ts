// Hoisting stops at the function boundary: a nested body's `var` must not leak
// into the enclosing one.
export function outer() {
function inner() {
var innerOnly = 1;
return innerOnly;
}
inner();
return innerOnly;
}
