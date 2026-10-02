export function f() {
  const nested: { inner: { v: number } } = { inner: { v: 1 } };
  nested["inner"]["v"] = "s";
}
