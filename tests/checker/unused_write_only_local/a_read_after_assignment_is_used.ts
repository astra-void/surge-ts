// @noUnusedLocals: true
export function f(data: Record<string, unknown>) {
let value: unknown;
for (const key in data) value = data[key];
return value;
}
