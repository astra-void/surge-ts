interface M {
  (a: string): number;
  (a: string, b: number): string;
}
declare const m: M;
export const r = m("a", 1);
