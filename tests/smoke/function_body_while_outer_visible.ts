function f(flag: boolean): string {
  const value = "hello";

  while (flag) {
    return value;
  }

  return "fallback";
}
