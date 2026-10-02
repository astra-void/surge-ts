// A `typeof` test on a property narrows the property, including the optional
// flag that carries its `undefined`.
interface O { p?: string }
declare function want(s: string): void;
export function f(o: O): void {
if (typeof o.p === "string") {
want(o.p);
}
if (typeof o.p === "undefined") {
return;
}
want(o.p);
}
