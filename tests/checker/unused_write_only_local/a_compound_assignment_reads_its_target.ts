// `x += 1` reads `x` before writing it, so the binding stays used.
// @noUnusedLocals: true
export function f() {
let total = 0;
total += 1;
return 1;
}
