function f(flag: boolean): string {
  while (flag) {
    const value = 1;
    return value;
  }

  return "fallback";
}
