declare const t1: [number, string, ...boolean[]];
declare let f10: (...x: [number, string, ...boolean[]]) => void;

export function calls(): void {
  f10(42, "hello");
  f10(42, "hello", true, false);
  f10(t1[0], t1[1], t1[2]);
  f10("hello", 42);
}

export function reads(): void {
  const a: string = t1[0];
  const b: string = t1[1];
  const c: string = t1[2];
}
