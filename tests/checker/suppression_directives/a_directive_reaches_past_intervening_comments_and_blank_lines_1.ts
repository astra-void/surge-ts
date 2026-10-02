export function f() {
  // @ts-expect-error - intentional
  // eslint-disable-next-line no-restricted-syntax
  const a: number = "s";
  return a;
}
