function f(flag: boolean, value: string): string {
  if (!flag && value === "hello") {
    return "ok";
  }

  return "fallback";
}

const a: number = -1;
const b = -"hello";
const c: string = !true;
