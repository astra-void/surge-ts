// A spread supplies as many arguments as its type holds, so counting it as one
// made every `f(...tuple)` a false TS2554.
declare function three(a: string, b: number, c: boolean): void;
const args = ["a", 1, true] as const;
export function f() { three(...args); }
