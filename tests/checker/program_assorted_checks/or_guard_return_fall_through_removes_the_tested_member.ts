// The same fall-through with an *early return* rather than `continue`: the
// composite-guard path never collected discriminant subjects, so `resolved`
// stayed the full union past the guard.
type Row =
| { kind: "resolved"; warnings: string[] }
| { kind: "unresolved"; reason: string };
export function f(r: Row, fatal: string): string[] {
if (r.kind === "unresolved" || fatal) { return []; }
return r.warnings;
}
