// tsc keeps a member whose property is OPTIONAL in the false branch — the key
// may legitimately be absent at runtime — so the else branch stays the full
// union and an own-member access on it is still reported.
type WithOpt = { a?: string; z: number };
type WithoutA = { b: string };
function f(v: WithOpt | WithoutA): number {
if ("a" in v) {
return v.z;
}
return v.z;
}
