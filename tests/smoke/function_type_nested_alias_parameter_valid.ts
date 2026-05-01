type Callback = (value: string) => number;

function use(callback: Callback): number {
  return callback("abc");
}

function getCount(value: string): number {
  return 1;
}

let result: number = use(getCount);
