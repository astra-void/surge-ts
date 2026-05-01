function length(value: string): number {
  return 1;
}

function apply(fn: (value: string) => number): number {
  return fn("abc");
}

let result: string = apply(length);
