// @noUnusedLocals: true
export function f(data: Record<string, unknown>) {
let writeOnly: unknown;
for (const key in data) writeOnly = data[key];
return 1;
}
