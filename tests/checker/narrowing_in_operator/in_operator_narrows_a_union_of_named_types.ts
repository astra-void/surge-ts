// Named aliases and interfaces reach narrowing as nominal references, and an
// alias to a union stays a nested union after peeling — both must be looked
// through or the guard narrows nothing (the trpc `Field | Fields` shape).
type O1 = { in: string; key: string };
type O2 = { key: string; map: string };
type O3 = { args?: number };
type Field = O1 | O2 | O3;
interface Fields { fields: string }
declare const c: Field | Fields;
export function a(): string | number | undefined {
if ("in" in c) {
return c.key;
}
if ("fields" in c) {
return c.fields;
}
if ("map" in c) {
return c.map;
}
return c.args;
}
