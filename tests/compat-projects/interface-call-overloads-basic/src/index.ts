interface Callable {
  (x: "A1"): string;
  (x: string): void;
}

declare const callable: Callable;

export function picks(): void {
  const a: string = callable("A1");
  const b: void = callable("zz");
  const c: number = callable("A1");
}
