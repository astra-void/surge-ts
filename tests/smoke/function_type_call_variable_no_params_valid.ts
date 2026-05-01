function length(): number {
  return 1;
}

let fn: () => number = length;
let result: number = fn();
