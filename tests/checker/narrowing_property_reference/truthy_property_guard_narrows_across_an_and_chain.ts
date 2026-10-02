// Every operand of an `&&` holds in the then-branch, so both properties narrow.
interface O { p?: string; q?: string }
declare function want(s: string): void;
export function f(o: O): void {
if (o.p && o.q) {
want(o.p);
want(o.q);
}
}
