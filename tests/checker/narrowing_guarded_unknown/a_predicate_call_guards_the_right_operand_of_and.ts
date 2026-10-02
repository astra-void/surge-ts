// A user-defined predicate guards the right operand of `&&` the same way
// `typeof`/`instanceof` already do, so the subject is no longer a genuine
// `unknown` receiver there.
function isObject(v: unknown): v is Record<string, unknown> {
return typeof v === "object" && v !== null;
}
export function f(err: unknown) {
return isObject(err) && typeof err["m"] === "string";
}
