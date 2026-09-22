declare let flag: boolean;
declare let count: number;
declare let text: string;
declare function sideEffect(): boolean;
declare function take(value: number): void;
declare const holder: { inner: string | number };
declare const shape: { present: number };
declare function isNumber(value: unknown): value is number;

flag = (count, text);
export const fromSequence: number = (sideEffect(), text);
take((sideEffect(), text));
export const operandsAreChecked: string = (shape.missing, text);

export function loop() {
  let index = 0;
  for (index = 0, flag = true; index < 3; index++, flag = !flag) {}
  return index;
}

if (typeof (sideEffect(), holder).inner === "number") {
  const narrowed: number = (sideEffect(), holder).inner;
  take(narrowed);
}
if (isNumber((sideEffect(), holder).inner)) {
  const narrowed: number = (sideEffect(), holder).inner;
  take(narrowed);
}
export const unnarrowed: number = (sideEffect(), holder).inner;
