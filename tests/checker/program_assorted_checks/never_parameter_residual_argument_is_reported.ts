// Only `"a"` is narrowed away, so the residual `"b"` is not assignable to
// `never`.
export function assertNever(_x: never): never {
  throw new Error();
}
export function f(value: "a" | "b"): string {
  if (value === "a") return "a";
  assertNever(value);
}
