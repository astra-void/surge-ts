interface Box<T> {
  inner: T;
}
declare function box<T>(value: T): Box<T>;

function make<T extends object>(value: T) {
  const boxed = box(value);
  return { ...boxed, extra: [value] };
}
const made = make({ id: 1 });
export const id: string = made.inner.id;
export const extra: number = made.extra;

function pair<T>(value: T) {
  if (!value) {
    return [value];
  }
  return [value, value];
}
export const first: number = pair("x")[0];
