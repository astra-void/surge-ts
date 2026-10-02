// `x op= v` is `x = x op v`: the operator's result type is what the write is
// checked against. Before this, no compound assignment was parsed at all.
export function f() {
  const arr: number[] = [1];
  arr[0] += 1;
  arr[0] += "s";
}
