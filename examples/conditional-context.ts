function take(value: string): string {
  return value;
}

const a: string = true ? "yes" : 1;
const b: string = true ? 1 : 2;
let c: string = "start";
c = true ? "yes" : 1;
const d = take(true ? "yes" : 1);
const e = true ? "yes" : 1;
