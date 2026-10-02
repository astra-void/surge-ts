export function f() {
  let mode: "omit" | "extend" = "omit";
  if (mode === "omit") { mode = "extend"; }
  else if (mode === "extend") { mode = "omit"; }
  return mode;
}
