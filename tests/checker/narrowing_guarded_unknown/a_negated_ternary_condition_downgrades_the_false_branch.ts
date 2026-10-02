// A negated condition guards the *false* branch, not the true one.
function isObject(v: unknown): v is Record<string, unknown> {
return typeof v === "object" && v !== null;
}
export function f(err: unknown) {
const value = !isObject(err) ? 0 : err["x"];
return value;
}
