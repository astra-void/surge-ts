// A generic predicate's type arguments are inferred from the tested argument.
type INVALID = { status: "aborted" };
type OK<T> = { status: "valid"; value: T };
type Sync<T> = OK<T> | INVALID;
declare function isValid<T>(x: Sync<T>): x is OK<T>;
export function f(r: Sync<string>): string | null {
return isValid(r) ? r.value : null;
}
