declare const o: { a: number };
export const x1 = o[0];
export const x2 = o["b"];

interface Item {
  a: number;
}
declare const item: Item;
declare const key: "zz";
export const x3 = item[1];
export const x4 = item[key];

declare const maybe: { a: number } | undefined;
export const x5 = maybe?.["b"];

export class Counter {
  static total = 1;
  count = 1;
}
declare const counter: Counter;
export const x6 = counter["total"];
export const x7 = counter.total;
