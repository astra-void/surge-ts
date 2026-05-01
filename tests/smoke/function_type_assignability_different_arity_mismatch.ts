function f(value: string): number {
  return 1;
}

let fn: (value: string, count: number) => number = f;
