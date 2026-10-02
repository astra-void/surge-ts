// The false branch must keep the members the predicate did *not* match.
type INVALID = { status: "aborted" };
type OK<T> = { status: "valid"; value: T };
type Sync<T> = OK<T> | INVALID;
declare function isValid<T>(x: Sync<T>): x is OK<T>;
export function f<O>(r: Sync<O>): string {
if (isValid(r)) {
return "ok";
}
return r.status;
}
