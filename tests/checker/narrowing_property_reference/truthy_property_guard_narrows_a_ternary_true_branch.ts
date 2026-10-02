// A ternary branches through the symbol-table narrowing path, not the scope stack.
interface O { p?: string }
declare function want(s: string): void;
export function f(o: O): void {
want(o.p ? o.p : "x");
}
