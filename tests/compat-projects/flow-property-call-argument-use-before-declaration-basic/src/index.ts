export function bad(obj: { method(value: number): void }) {
  obj.method(x);
  let x = 1;
}
