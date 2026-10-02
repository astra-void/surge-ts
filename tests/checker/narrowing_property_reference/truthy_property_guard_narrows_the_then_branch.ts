// `if (o.p)` narrows the property inside the pushed then-branch scope, the way a
// bare-identifier truthy guard already did.
interface O { p?: string }
declare function want(s: string): void;
export function f(o: O): void {
if (o.p) {
want(o.p);
}
}
