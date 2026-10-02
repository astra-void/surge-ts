// A `do … while` body runs before the condition, so what it assigns is
// definitely assigned afterwards — the plain `while` lowering would report
// TS2454 here.
export function f() {
  let y: string;
  do { y = "a"; } while (false);
  return y.length;
}
