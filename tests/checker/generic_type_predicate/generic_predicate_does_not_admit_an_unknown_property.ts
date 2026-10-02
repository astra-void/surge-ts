// Narrowing must not invent members: a property the matched member does not
// declare still reports.
type INVALID = { status: "aborted" };
type OK<T> = { status: "valid"; value: T };
type Sync<T> = OK<T> | INVALID;
declare function isValid<T>(x: Sync<T>): x is OK<T>;
export function f<O>(r: Sync<O>): unknown {
if (isValid(r)) {
return r.missing;
}
return null;
}
