function isS(x: number): x is string { return true; }
function isN(x: number | string): x is number { return true; }
function ok(x: unknown): x is string { return true; }
function isA(x: { a: number }): x is { b: string } { return true; }
declare function decl(x: number): x is string;
const arrow = (x: number): x is string => true;
class C { m(x: number): x is string { return true; } }
function assertsS(x: number): asserts x is string {}
function multi(a: string, b: number): b is boolean { return true; }
function opt(x?: string): x is string { return true; }
function lit(x: "a" | "b"): x is "a" { return true; }
export {}
