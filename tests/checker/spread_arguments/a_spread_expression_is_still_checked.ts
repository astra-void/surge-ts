// The spread's own expression is code: it was dropped as `Unknown` at parse
// time, so nothing inside it was ever checked.
declare function three(a: string, b: number, c: boolean): void;
export function f() { three(...missingName); }
