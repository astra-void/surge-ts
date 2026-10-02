// `o.p = v` reads `o`.
// @noUnusedLocals: true
export function f() {
const holder = { value: 0 };
holder.value = 1;
return 1;
}
