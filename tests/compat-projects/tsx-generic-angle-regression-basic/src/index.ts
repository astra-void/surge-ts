function identity<T>(value: T): T {
  return value;
}

const n: number = identity<number>(1);
const s: string = identity<string>("ok");
const less = 1 < 2;
const arr: Array<number> = [1, 2, 3];

export { n, s, less, arr };
