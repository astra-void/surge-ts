export function need(a: string): void {}
export function f() {
  // @ts-expect-error
  need();
  need();
}
