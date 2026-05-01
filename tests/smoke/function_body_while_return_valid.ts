function f(flag: boolean): string {
  while (flag) {
    const value = "hello";
    return value;
  }

  return "fallback";
}
