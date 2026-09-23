class Box {
  constructor(value: number) {}
}
interface Shelf {
  1: typeof Box;
}
declare const rows: Shelf[][];

const built = new rows["a-b"][1](1);
const stored = rows["a-b"][1];
const again = new stored("not checked");
export { built, again };
