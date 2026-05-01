function f(flag: boolean): string {
  if (flag) {
    const value = 1;
    return value;
  }

  return "fallback";
}
