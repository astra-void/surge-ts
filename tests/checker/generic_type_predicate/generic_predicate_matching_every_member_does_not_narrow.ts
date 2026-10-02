// A predicate whose target matches *every* member proves nothing and must leave
// the subject alone rather than collapsing it to an `Any`-filled predicate.
type Box<T> = { value: T };
declare function isBox<T>(x: Box<T>): x is Box<T>;
export function f(r: Box<string>): number {
if (isBox(r)) {
return r.value;
}
return 0;
}
