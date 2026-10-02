// The guard also has to survive alongside a syntactic guard earlier in the
// chain, which is the case that already produced a narrowed table.
function isObject(v: unknown): v is Record<string, unknown> {
return typeof v === "object" && v !== null;
}
export function f(a: unknown, b: unknown) {
return typeof a === "string" && isObject(b) && !!b["m"];
}
