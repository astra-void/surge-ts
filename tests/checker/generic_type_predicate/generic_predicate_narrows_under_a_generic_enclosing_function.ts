// The written parameter is an alias whose expansion is a union, so `T` cannot be
// aligned and stays unbound. Narrowing still selects the `OK` member.
type INVALID = { status: "aborted" };
type OK<T> = { status: "valid"; value: T };
type Sync<T> = OK<T> | INVALID;
declare function isValid<T>(x: Sync<T>): x is OK<T>;
export function f<O>(r: Sync<O>): O | null {
return isValid(r) ? r.value : null;
}
