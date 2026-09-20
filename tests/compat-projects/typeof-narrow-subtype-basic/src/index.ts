declare let wide: {};
declare let stringy: { toString(): string };
declare let shaped: { q: number };

export function narrows(): void {
  if (typeof wide === "number") {
    const a: string = wide;
  }
  if (typeof stringy === "string") {
    const b: number = stringy;
  }
  if (typeof shaped === "object") {
    const c: string = shaped;
  }
  if (typeof shaped === "number") {
    const d: string = shaped;
  }
}
