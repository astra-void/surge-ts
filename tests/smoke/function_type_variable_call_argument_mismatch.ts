function measure(value: string): number {
  return 1;
}

let fn: (value: string) => number = measure;
let result: number = fn(123);
