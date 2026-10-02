export function f(key: string) {
  const rec: Record<string, number> = {};
  rec["a"] = 1;
  rec[key] = "s";
}
