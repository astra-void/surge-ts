function f(flag: boolean): string {
  let value = "hello";

  while (flag) {
    value = "world";
  }

  return value;
}
