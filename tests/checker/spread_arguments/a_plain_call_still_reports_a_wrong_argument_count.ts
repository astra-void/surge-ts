// A call with no spread still reports a real arity mismatch.
declare function three(a: string, b: number, c: boolean): void;
export function f() { three("a", 1); }
