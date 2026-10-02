// `while (true)` enters its body unconditionally, so what the body assigns is
// assigned after the loop.
export function f() {
  let x: number;
  outer: while (true) { x = 1; break outer; }
  return x;
}
