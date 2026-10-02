// The falsy complement is deliberately unmodelled, but the guard must not leak
// out of its branch: the property is still optional after the `if` and in the
// `else`.
interface O { p?: string }
declare function want(s: string): void;
export function after(o: O): void {
if (o.p) {
}
want(o.p);
}
