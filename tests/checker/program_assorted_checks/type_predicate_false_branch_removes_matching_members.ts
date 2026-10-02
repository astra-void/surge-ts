// The fall-through of an early-returning predicate guard removes the matching
// members (`if (isStr(x)) return …;` leaves `number`).
function isStr(v: unknown): v is string {
return typeof v === "string";
}
function f(x: string | number): number {
if (isStr(x)) return x.length;
return x * 2;
}
