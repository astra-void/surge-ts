function take(value: string): string {
  return value;
}

const a: string = true ? (false ? "yes" : 1) : "no";
const b: string = true ? (false ? 1 : 2) : "no";
let c: string = "start";
c = true ? (false ? "yes" : 1) : "no";
const d = take(true ? (false ? "yes" : 1) : "no");
const e = true ? (false ? "yes" : 1) : "no";
