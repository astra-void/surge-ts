// A predicate written as a `const` annotation carries no collected signature
// unless the annotation is kept for it.
type INVALID = { status: "aborted" };
type OK<T> = { status: "valid"; value: T };
type Sync<T> = OK<T> | INVALID;
declare const isValid: (x: Sync<string>) => x is OK<string>;
export function f(r: Sync<string>): string | null {
return isValid(r) ? r.value : null;
}
