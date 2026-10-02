export function f() {
type T = typeof missing;
const x: T = null as any;
return x;
}
