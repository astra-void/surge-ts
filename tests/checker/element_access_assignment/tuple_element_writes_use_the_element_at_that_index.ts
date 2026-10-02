export function f() {
  const t: [number, string] = [1, "a"];
  t[0] = 2;
  t[1] = "b";
  t[0] = "s";
}
