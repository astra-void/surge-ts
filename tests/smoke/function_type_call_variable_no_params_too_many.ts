function measure(): number {
  return 1;
}

let fn: () => number = measure;
let result: number = fn(1);
