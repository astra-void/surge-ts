// A `||` proves nothing about either operand in the true branch.
interface O { p?: string; q?: string }
declare function want(s: string): void;
export function f(o: O): void {
if (o.p || o.q) {
want(o.q);
}
}
