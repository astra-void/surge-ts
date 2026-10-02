// Arguments beside a spread keep their own contextual check.
declare const rest: [number, boolean];
declare function three(a: string, b: number, c: boolean): void;
export function f() { three(1, ...rest); }
