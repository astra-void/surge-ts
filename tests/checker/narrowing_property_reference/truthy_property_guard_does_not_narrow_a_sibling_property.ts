// Guarding one property proves nothing about a sibling.
interface O { p?: string; q?: string }
declare function want(s: string): void;
export function f(o: O): void {
if (o.p) {
want(o.q);
}
}
