// @noUnusedParameters: true
// The bodyless overload signature's parameters must not be flagged; only the
// implementation is checked (and here `a` is used).
export function f(a: string): string;
export function f(a: number): string;
export function f(a: string | number): string { return String(a); }
