function f(flag: boolean): string {
  return flag ? "yes" : "no";
}

const ok: string = true ? "yes" : "no";
const bad: number = true ? "yes" : "no";
const mixed = true ? "yes" : 1;
