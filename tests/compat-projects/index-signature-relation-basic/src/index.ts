interface Obj {
  hello: string;
  world: number;
}
type Lit = { hello: string; world: number };
interface StrNum {
  [key: string]: number;
}
interface StrStr {
  [key: string]: string;
}
interface StrAny {
  [key: string]: any;
}
interface NumStr {
  [key: number]: string;
}

export function targets(obj: Obj, lit: Lit, strNum: StrNum, strAny: StrAny, numStr: NumStr): void {
  strNum = obj;
  strNum = lit;
  strNum = { a: 1 };
  strNum = { a: "x" };
  strAny = obj;
  strNum = numStr;
}

export function sources(obj: Obj, strNum: StrNum, strStr: StrStr): void {
  obj = strNum;
  obj = strStr;
}

export function records(wide: Record<string, string>, narrow: Record<"a", string>): void {
  narrow = wide;
}
