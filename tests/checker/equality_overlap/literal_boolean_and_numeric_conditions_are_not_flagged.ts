// tsc exempts boolean and numeric literal conditions from the always-truthy /
// always-falsy checks, so the idiomatic `while (true)` stays clean.
export function f() {
while (true) {
break;
}
if (1) {
}
if (0) {
}
if (false) {
}
}
