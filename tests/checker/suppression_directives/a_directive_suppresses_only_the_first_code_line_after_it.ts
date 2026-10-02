export function f() {
  // @ts-expect-error - intentional
  const a: number = "s";
  const b: number = "s";
  return a + b;
}
