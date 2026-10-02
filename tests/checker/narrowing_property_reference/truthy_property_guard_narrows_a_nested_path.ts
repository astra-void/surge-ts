// The guard reaches through a nested path, not just one property level.
interface O { nested: { r?: string } }
declare function want(s: string): void;
export function f(o: O): void {
if (o.nested.r) {
want(o.nested.r);
}
}
