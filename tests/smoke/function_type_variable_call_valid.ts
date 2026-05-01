function length(value: string): number {
  return 1;
}

let fn: (value: string) => number = length;
let result: number = fn("abc");
