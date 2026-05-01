function f(flag: boolean): string {
  const value = "hello";

  while (flag) {
    value = "world";
  }

  return value;
}
