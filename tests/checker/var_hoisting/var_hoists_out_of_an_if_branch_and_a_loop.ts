// Visible, but not definitely assigned on every path.
declare const cond: boolean;
export function f() {
if (cond) {
var fromBranch = 1;
}
for (const _ of [1]) {
var fromLoop = 2;
}
return fromBranch + fromLoop;
}
