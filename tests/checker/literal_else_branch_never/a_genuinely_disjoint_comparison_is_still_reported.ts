export function f(mode: "omit" | "extend") {
  if (mode === "omit") { return 0; }
  return mode === "nope" ? 1 : 2;
}
