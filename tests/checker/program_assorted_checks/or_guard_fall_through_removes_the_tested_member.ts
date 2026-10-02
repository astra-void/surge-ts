// The fall-through of `A || B` is `!A && !B`: a discriminant disjunct must be
// removed from the union, not selected. Treating the guard as one atom left the
// tested member in place.
type Row =
| { kind: "resolved"; warnings: string[] }
| { kind: "unresolved"; reason: string };
declare function fatal(): string | null;
export function f(rows: Row[]) {
for (const r of rows) {
if (r.kind === "unresolved" || fatal()) { continue; }
console.log(r.warnings.length);
}
}
