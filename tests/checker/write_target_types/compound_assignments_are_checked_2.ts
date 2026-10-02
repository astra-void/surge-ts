// `x op= v` is `x = x op v`: the operator's result type is what the write is
// checked against. Before this, no compound assignment was parsed at all.
export function f() {
  let text = "a";
  text += 1;
  let total = 0;
  total += "s";
  return text + total;
}
